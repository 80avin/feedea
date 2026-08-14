use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rssea::api;
use rssea::app_db;
use rssea::auth;
use rssea::config::Config;
use rssea::engine::Engine;
use rssea::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test Feed</title>
    <link>http://127.0.0.1/</link>
    <description>test</description>
    <item>
      <title>Article Alpha</title>
      <link>http://example.com/alpha</link>
      <guid isPermaLink="false">alpha-1</guid>
      <pubDate>Mon, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>Alpha body.</description>
    </item>
    <item>
      <title>Article Beta</title>
      <link>http://example.com/beta</link>
      <guid isPermaLink="false">beta-1</guid>
      <pubDate>Tue, 12 Aug 2026 11:30:00 GMT</pubDate>
      <description>Beta body.</description>
    </item>
  </channel>
</rss>
"#;

mod feed_server;

async fn spawn_app() -> (String, axum::Router, feed_server::FeedServer, Arc<Mutex<app_db::AppDb>>) {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let server = feed_server::FeedServer::start(RSS.to_string(), 12);
    let dir = std::env::temp_dir().join(format!(
        "rssea-sources-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let config = Config { data_dir: dir, host: "127.0.0.1".into(), port: 0 };
    let engine = Engine::new(&config).await.unwrap();
    let mut db = app_db::open(&config.data_dir).unwrap();
    db.set_password_hash(&auth::hash_password("test-pass").unwrap()).unwrap();
    let app_db = Arc::new(Mutex::new(db));
    let router = api::router(AppState { engine: engine.clone(), app_db: app_db.clone(), allow_private_proxy: false });
    (server.url.clone(), router, server, app_db)
}

async fn login_cookie(app: &axum::Router) -> String {
    let resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/login")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"password":"test-pass"}"#))
            .unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    resp.headers().get(axum::http::header::SET_COOKIE)
        .expect("login should set a session cookie")
        .to_str().unwrap().to_string()
}

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_string()
}

async fn get_groups(app: &axum::Router, cookie: &str) -> Vec<serde_json::Value> {
    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/sources")
            .header(axum::http::header::COOKIE, cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    value["groups"].as_array().unwrap().clone()
}

fn find_group<'a>(groups: &'a [serde_json::Value], category_id: &str) -> Option<&'a serde_json::Value> {
    groups.iter().find(|g| g["category_id"] == category_id)
}

fn group_has_feed(group: &serde_json::Value, feed_id: &str) -> bool {
    group["feeds"].as_array().unwrap().iter().any(|f| f["id"] == feed_id)
}

fn esc(id: &str) -> String {
    let mut out = String::new();
    for b in id.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[tokio::test]
async fn sources_requires_auth() {
    let (_feed_url, app, _server, _db) = spawn_app().await;
    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/sources").body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sources_crud_and_delete_prunes_saved() {
    let (feed_url, app, _server, app_db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(format!(r#"{{"url":"{feed_url}"}}"#))).unwrap())
        .await.unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);
    let body = add_resp.into_body().collect().await.unwrap().to_bytes();
    let feed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let feed_id = feed["id"].as_str().unwrap().to_string();
    assert_eq!(feed["title"], "Test Feed");

    let groups = get_groups(&app, &cookie).await;
    let toplevel = find_group(&groups, "NewsFlash.Toplevel").expect("feed should be grouped under toplevel");
    assert!(group_has_feed(toplevel, &feed_id), "feed should be listed");

    let rename_resp = app.clone()
        .oneshot(Request::builder().method("PATCH")
            .uri(format!("/api/sources/{}", esc(&feed_id)))
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"title":"Renamed"}"#)).unwrap())
        .await.unwrap();
    assert_eq!(rename_resp.status(), StatusCode::OK);

    let groups = get_groups(&app, &cookie).await;
    let toplevel = find_group(&groups, "NewsFlash.Toplevel").unwrap();
    let renamed = toplevel["feeds"].as_array().unwrap().iter().find(|f| f["id"] == feed_id).unwrap();
    assert_eq!(renamed["title"], "Renamed");

    let read_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri(format!("/api/sources/{}/read", esc(&feed_id)))
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(read_resp.status(), StatusCode::OK);

    let groups = get_groups(&app, &cookie).await;
    let toplevel = find_group(&groups, "NewsFlash.Toplevel").unwrap();
    let read_feed = toplevel["feeds"].as_array().unwrap().iter().find(|f| f["id"] == feed_id).unwrap();
    assert_eq!(read_feed["unread_count"], 0);

    let refresh_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri("/api/sources/refresh-all")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(refresh_resp.status(), StatusCode::OK);

    let list_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?offset=0&limit=10")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let body = list_resp.into_body().collect().await.unwrap().to_bytes();
    let articles: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = articles.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let article_id = arr[0]["id"].as_str().unwrap().to_string();

    let save_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri(format!("/api/articles/{article_id}/save"))
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"note":"must prune"}"#)).unwrap())
        .await.unwrap();
    assert_eq!(save_resp.status(), StatusCode::OK);

    let saved_resp = app.clone()
        .oneshot(Request::builder().uri("/api/saved")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(saved_resp.status(), StatusCode::OK);
    let body = saved_resp.into_body().collect().await.unwrap().to_bytes();
    let saved: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(saved["total"], 1, "saved article should be listed before delete");

    let delete_resp = app.clone()
        .oneshot(Request::builder().method("DELETE")
            .uri(format!("/api/sources/{}", esc(&feed_id)))
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::OK);

    let saved_resp = app.clone()
        .oneshot(Request::builder().uri("/api/saved")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(saved_resp.status(), StatusCode::OK);
    let body = saved_resp.into_body().collect().await.unwrap().to_bytes();
    let saved: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(saved["total"], 0, "saved article must be pruned when its feed is deleted");

    let groups = get_groups(&app, &cookie).await;
    assert!(groups.iter().all(|g| !group_has_feed(g, &feed_id)), "deleted feed must not be listed");

    let db = app_db.lock().await;
    let saved_count: i64 = db.conn
        .query_row("SELECT COUNT(*) FROM saved WHERE article_id = ?1", rusqlite::params![article_id], |r| r.get(0))
        .unwrap();
    assert_eq!(saved_count, 0, "app-db saved row must be gone after delete");
}

#[tokio::test]
async fn sources_can_move_between_categories() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let create_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/categories")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"name":"Tech"}"#)).unwrap())
        .await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let body = create_resp.into_body().collect().await.unwrap().to_bytes();
    let tech_id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["category_id"].as_str().unwrap().to_string();

    let add_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(format!(r#"{{"url":"{feed_url}"}}"#))).unwrap())
        .await.unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);
    let body = add_resp.into_body().collect().await.unwrap().to_bytes();
    let feed_id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"].as_str().unwrap().to_string();

    let move_resp = app.clone()
        .oneshot(Request::builder().method("PATCH")
            .uri(format!("/api/sources/{}", esc(&feed_id)))
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(format!(r#"{{"category_id":"{tech_id}"}}"#))).unwrap())
        .await.unwrap();
    assert_eq!(move_resp.status(), StatusCode::OK);

    let groups = get_groups(&app, &cookie).await;
    let tech = find_group(&groups, &tech_id).expect("feed should be grouped under Tech");
    assert!(group_has_feed(tech, &feed_id), "feed should appear in Tech group");
    assert!(find_group(&groups, "NewsFlash.Toplevel").is_none_or(|g| !group_has_feed(g, &feed_id)));
}

#[tokio::test]
async fn discover_returns_title_for_feed_url_without_adding() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources/discover")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(format!(r#"{{"url":"{feed_url}"}}"#))).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["title"], "Test Feed");
    assert_eq!(value["feed_url"], feed_url.as_str());
    assert_eq!(value["alternatives"].as_array().unwrap().len(), 0);

    let groups = get_groups(&app, &cookie).await;
    assert!(groups.iter().all(|g| g["feeds"].as_array().unwrap().is_empty()), "discover must not add a feed");
}

#[tokio::test]
async fn import_and_export_opml() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let opml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>My Feeds</title></head>
  <body>
    <outline text="Test Feed" title="Test Feed" type="rss" xmlUrl="{feed_url}"/>
  </body>
</opml>"#);
    let body = serde_json::json!({ "opml": opml }).to_string();

    let import_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources/import-opml")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(body)).unwrap())
        .await.unwrap();
    assert_eq!(import_resp.status(), StatusCode::OK);
    let body = import_resp.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["imported"], true);

    let groups = get_groups(&app, &cookie).await;
    let toplevel = find_group(&groups, "NewsFlash.Toplevel").expect("imported feed should be grouped under toplevel");
    assert!(group_has_feed(toplevel, &feed_url), "imported feed should be listed");

    let export_resp = app.clone()
        .oneshot(Request::builder().uri("/api/sources/export-opml")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(export_resp.status(), StatusCode::OK);
    assert_eq!(export_resp.headers().get(axum::http::header::CONTENT_TYPE).unwrap(), "text/xml");
    let body = export_resp.into_body().collect().await.unwrap().to_bytes();
    let xml = String::from_utf8(body.to_vec()).unwrap();
    assert!(xml.contains(&feed_url), "exported opml should contain the feed url");
    assert!(!xml.trim().is_empty());
}
