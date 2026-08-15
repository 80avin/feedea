use axum::body::Body;
use axum::http::{Request, StatusCode};
use feedea::AppState;
use feedea::api;
use feedea::app_db;
use feedea::config::Config;
use feedea::engine::Engine;
use http_body_util::BodyExt;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use tokio::sync::Mutex;
use tower::ServiceExt;

const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D', b'R',
];

struct ByteServer {
    url: String,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ByteServer {
    fn start(png: &'static [u8]) -> ByteServer {
        ByteServer::start_with_status(png, 200)
    }

    fn start_with_status(png: &'static [u8], status: u16) -> ByteServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/img.png");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let handle = thread::spawn(move || {
            let started = std::time::Instant::now();
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let reason = if status == 200 { "OK" } else { "Not Found" };
                        let header = format!(
                            "HTTP/1.1 {status} {reason}\r\nContent-Type: image/png\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n",
                            status = status,
                            reason = reason,
                            len = png.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.write_all(png);
                        let _ = stream.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if started.elapsed() > std::time::Duration::from_secs(60) {
                            break;
                        }
                        thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        ByteServer {
            url,
            stop,
            handle: Some(handle),
        }
    }

    fn start_huge_content_length() -> ByteServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/img.png");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let handle = thread::spawn(move || {
            let started = std::time::Instant::now();
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let header = "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 99999999999\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(header.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if started.elapsed() > std::time::Duration::from_secs(60) {
                            break;
                        }
                        thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        ByteServer {
            url,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for ByteServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

async fn spawn_app(allow_private_proxy: bool) -> axum::Router {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "feedea-proxy-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let config = Config {
        data_dir: dir,
        host: "127.0.0.1".into(),
        port: 0,
        allow_private_proxy: false,
    };
    let engine = Engine::new(&config).await.unwrap();
    let db = app_db::open(&config.data_dir).unwrap();
    let app_db = Arc::new(Mutex::new(db));
    api::router(AppState {
        engine: engine.clone(),
        app_db: app_db.clone(),
        allow_private_proxy,
    })
}

#[tokio::test]
async fn proxy_serves_upstream_image_bytes() {
    let server = ByteServer::start(PNG);
    let app = spawn_app(true).await;
    let uri = format!("/img?u={}", url_encode(&server.url));
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .unwrap(),
        "image/png"
    );
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], PNG);
}

#[tokio::test]
async fn proxy_rejects_non_http_scheme() {
    let app = spawn_app(false).await;
    let uri = format!("/img?u={}", url_encode("ftp://example.com/img.png"));
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn proxy_rejects_missing_u() {
    let app = spawn_app(false).await;
    let resp = app
        .clone()
        .oneshot(Request::builder().uri("/img").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn proxy_upstream_404_is_502() {
    let server = ByteServer::start_with_status(PNG, 404);
    let app = spawn_app(true).await;
    let uri = format!("/img?u={}", url_encode(&server.url));
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn proxy_unreachable_upstream_is_502() {
    let app = spawn_app(true).await;
    let uri = format!("/img?u={}", url_encode("http://127.0.0.1:1/nope.png"));
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn proxy_rejects_private_target() {
    let server = ByteServer::start(PNG);
    let app = spawn_app(false).await;
    let uri = format!("/img?u={}", url_encode(&server.url));
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn proxy_rejects_oversized_content_length() {
    let server = ByteServer::start_huge_content_length();
    let app = spawn_app(true).await;
    let uri = format!("/img?u={}", url_encode(&server.url));
    let resp = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
