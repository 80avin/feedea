use axum::body::Body;
use axum::http::{Request, StatusCode};
use feedea::AppState;
use feedea::api;
use feedea::app_db;
use feedea::auth;
use feedea::config::Config;
use feedea::engine::Engine;
use http_body_util::BodyExt;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use tokio::sync::Mutex;
use tower::ServiceExt;

const N_CATS: usize = 24;
const N_UNCATEGORIZED: usize = 4;
const N_ITEMS: usize = 40;

fn rss_body(feed_label: &str) -> String {
    let items = (1..=N_ITEMS)
        .map(|i| {
            let day = 1 + (i % 27);
            format!(
                "<item><title>{feed_label} item {i}</title><link>http://example.com/{feed_label}/{i}</link>\
                 <guid isPermaLink=\"false\">{feed_label}-{i}</guid>\
                 <pubDate>Mon, {day} Aug 2026 10:00:00 GMT</pubDate>\
                 <description>{feed_label} body {i}.</description></item>"
            )
        })
        .collect::<Vec<String>>()
        .join("");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\"><channel>\
         <title>{feed_label}</title><link>http://127.0.0.1/</link><description>x</description>\
         {items}</channel></rss>"
    )
}

struct PerfServer {
    addr: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl PerfServer {
    fn start(rss: String) -> PerfServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            rss.len(),
                            rss
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        PerfServer {
            addr,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for PerfServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

async fn spawn_app() -> (String, axum::Router, PerfServer) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let server = PerfServer::start(rss_body("perf"));
    let url = format!("http://{}", server.addr);
    let dir = std::env::temp_dir().join(format!(
        "feedea-perf-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let config = Config {
        data_dir: dir,
        host: "127.0.0.1".into(),
        port: 0,
        allow_private_proxy: false,
        memory_trim_minutes: feedea::memtrim::DEFAULT_TRIM_MINUTES,
        memory_arena_max: feedea::memtrim::DEFAULT_ARENA_MAX,
    };
    let engine = Engine::new(&config).await.unwrap();
    let mut db = app_db::open(&config.data_dir).unwrap();
    db.set_password_hash(&auth::hash_password("test-pass").unwrap())
        .unwrap();
    let app_db = Arc::new(Mutex::new(db));
    let router = api::router(AppState {
        engine: engine.clone(),
        app_db,
        allow_private_proxy: false,
    });
    (url, router, server)
}

async fn login_cookie(app: &axum::Router) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"password":"test-pass"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    resp.headers()
        .get(axum::http::header::SET_COOKIE)
        .expect("login should set a session cookie")
        .to_str()
        .unwrap()
        .to_string()
}

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_string()
}

#[tokio::test]
#[ignore]
async fn overview_latency_benchmark() {
    let (base_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    for i in 0..N_CATS {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/categories")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(format!(r#"{{"name":"Cat {i}"}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let cat_id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["category_id"]
            .as_str()
            .unwrap()
            .to_string();
        let url = format!("{base_url}/c{i}.xml");
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(format!(
                        r#"{{"url":"{url}","title":"Feed {i}","category_id":"{cat_id}"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "add feed {i} failed");
    }

    for i in 0..N_UNCATEGORIZED {
        let url = format!("{base_url}/u{i}.xml");
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(format!(
                        r#"{{"url":"{url}","title":"Uncat {i}"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "add uncategorized feed {i} failed"
        );
    }

    let mut cards = 0usize;
    let mut samples: Vec<f64> = Vec::new();
    for run in 0..23 {
        let t0 = std::time::Instant::now();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/overview")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json["all"]["total_count"].as_i64().unwrap() >= (N_CATS * N_ITEMS) as i64,
            "articles must be ingested before timing"
        );
        cards = json["cards"].as_array().unwrap().len();
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        if run >= 3 {
            samples.push(elapsed);
        }
    }
    assert!(
        cards >= N_CATS,
        "expected at least {N_CATS} cards, got {cards}"
    );
    samples.sort_by(|a, b| a.total_cmp(b));
    let n = samples.len();
    let median = if n.is_multiple_of(2) {
        (samples[n / 2 - 1] + samples[n / 2]) / 2.0
    } else {
        samples[n / 2]
    };
    let min = samples[0];
    let mean: f64 = samples.iter().sum::<f64>() / n as f64;
    println!(
        "OVERVIEW_PERF cards={cards} min_ms={min:.2} median_ms={median:.2} mean_ms={mean:.2} samples={n}"
    );
}
