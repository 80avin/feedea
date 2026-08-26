use axum::body::Body;
use axum::http::{Request, StatusCode};
use feedea::AppState;
use feedea::api;
use feedea::app_db;
use feedea::auth;
use feedea::config::Config;
use feedea::engine::Engine;
use http_body_util::BodyExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Overview Feed</title>
    <link>http://127.0.0.1/</link>
    <description>overview</description>
    <item>
      <title>Overview Alpha</title>
      <link>http://example.com/alpha</link>
      <guid isPermaLink="false">overview-alpha-1</guid>
      <pubDate>Mon, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>Alpha body.</description>
    </item>
    <item>
      <title>Overview Beta</title>
      <link>http://example.com/beta</link>
      <guid isPermaLink="false">overview-beta-1</guid>
      <pubDate>Tue, 12 Aug 2026 11:30:00 GMT</pubDate>
      <description>Beta body.</description>
    </item>
  </channel>
</rss>
"#;

const OLD_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Old Feed</title>
    <link>http://127.0.0.1/</link>
    <description>old</description>
    <item>
      <title>Old Article</title>
      <link>http://example.com/old</link>
      <guid isPermaLink="false">old-1</guid>
      <pubDate>Mon, 12 Jan 2026 10:00:00 GMT</pubDate>
      <description>Old body.</description>
    </item>
    <item>
      <title>Older Article</title>
      <link>http://example.com/older</link>
      <guid isPermaLink="false">old-2</guid>
      <pubDate>Sun, 11 Jan 2026 10:00:00 GMT</pubDate>
      <description>Older body.</description>
    </item>
  </channel>
</rss>
"#;

fn fresh_rss(n: usize) -> String {
    let items: String = (0..n)
        .map(|i| {
            let day = 1 + (i % 27);
            format!(
                "<item><title>Fresh {i}</title><link>http://example.com/f/{i}</link>\
                 <guid isPermaLink=\"false\">f-{i}</guid>\
                 <pubDate>Mon, {day} Aug 2026 10:00:00 GMT</pubDate>\
                 <description>Fresh body {i}.</description></item>"
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\"><channel>\
         <title>Fresh</title><link>http://127.0.0.1/</link><description>fresh</description>\
         {items}</channel></rss>"
    )
}

mod feed_server;

async fn spawn_app() -> (String, axum::Router, feed_server::FeedServer) {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let server = feed_server::FeedServer::start(RSS.to_string(), 10);
    let dir = std::env::temp_dir().join(format!(
        "feedea-overview-test-{}-{}",
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
    let mut db = app_db::open(&config.data_dir).unwrap();
    db.set_password_hash(&auth::hash_password("test-pass").unwrap())
        .unwrap();
    let app_db = Arc::new(Mutex::new(db));
    let router = api::router(AppState {
        engine: engine.clone(),
        app_db,
        allow_private_proxy: false,
    });
    (server.url.clone(), router, server)
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

async fn create_category(app: &axum::Router, cookie: &str, body: &str) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/categories")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice::<serde_json::Value>(&body).unwrap()["category_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn add_feed(
    app: &axum::Router,
    cookie: &str,
    feed_url: &str,
    title: &str,
    category_id: Option<&str>,
) {
    let category = category_id
        .map(|c| format!(r#","category_id":"{c}""#))
        .unwrap_or_default();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::from(format!(
                    r#"{{"url":"{feed_url}","title":"{title}"{category}}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

async fn refresh_all(app: &axum::Router, cookie: &str) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/refresh-all")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

async fn overview(app: &axum::Router, cookie: &str) -> serde_json::Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/overview")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn overview_requires_auth() {
    let (_feed_url, app, _server) = spawn_app().await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/overview")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn overview_parent_without_direct_feeds_gets_no_card() {
    let (feed_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let parent_id = create_category(&app, &cookie, r#"{"name":"Parent"}"#).await;
    let child_id = create_category(
        &app,
        &cookie,
        &format!(r#"{{"name":"Child","parent_id":"{parent_id}"}}"#),
    )
    .await;

    add_feed(&app, &cookie, &feed_url, "Nested Feed", Some(&child_id)).await;
    refresh_all(&app, &cookie).await;

    let json = overview(&app, &cookie).await;
    let cards = json["cards"].as_array().unwrap();
    assert!(
        cards.iter().all(|c| c["category_id"] != parent_id),
        "container category without direct feeds must not get a card: {cards:?}"
    );
    assert!(cards.iter().all(|c| c["name"] != "Parent"));
    let child = cards
        .iter()
        .find(|card| card["category_id"] == child_id)
        .expect("child card should exist");
    assert!(child["total_count"].as_i64().unwrap() >= 1);
    assert!(child["unread_count"].as_i64().unwrap() >= 1);
}

#[tokio::test]
async fn overview_no_toplevel_card_without_uncategorized_feeds() {
    let (feed_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let cat_id = create_category(&app, &cookie, r#"{"name":"Cat"}"#).await;
    add_feed(&app, &cookie, &feed_url, "Categorized", Some(&cat_id)).await;
    refresh_all(&app, &cookie).await;

    let json = overview(&app, &cookie).await;
    let cards = json["cards"].as_array().unwrap();
    assert!(
        cards.iter().all(|c| c["name"] != "NewsFlash.Toplevel"),
        "toplevel card must not appear when no feed maps directly to it"
    );
    let cat = cards
        .iter()
        .find(|c| c["category_id"] == cat_id)
        .expect("categorized feed's card should exist");
    assert!(cat["total_count"].as_i64().unwrap() >= 2);
    assert!(json["all"]["total_count"].as_i64().unwrap() >= 2);
}

#[tokio::test]
async fn overview_returns_per_category_cards_and_all_totals() {
    let (feed_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(format!(
                    r#"{{"url":"{feed_url}","title":"Overview Feed"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    let refresh_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/refresh-all")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refresh_resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/overview")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let cards = json["cards"].as_array().expect("cards should be an array");
    let toplevel = cards
        .iter()
        .find(|card| card["name"] == "NewsFlash.Toplevel")
        .expect("cards should contain a NewsFlash.Toplevel card");
    assert!(toplevel["total_count"].as_i64().unwrap() >= 2);
    let items = toplevel["items"]
        .as_array()
        .expect("card items should be an array");
    assert!(!items.is_empty());
    assert_eq!(toplevel["unread_count"].as_i64().unwrap(), 2);

    assert!(json["all"]["total_count"].as_i64().unwrap() >= 2);
    assert!(json["all"]["unread_count"].as_i64().unwrap() >= 2);
}

#[tokio::test]
async fn overview_shows_full_category_path() {
    let (feed_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let parent_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/categories")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(r#"{"name":"Top"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let parent_id = serde_json::from_slice::<serde_json::Value>(
        &parent_resp.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap()["category_id"]
        .as_str()
        .unwrap()
        .to_string();

    let child_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/categories")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(format!(
                    r#"{{"name":"Sub","parent_id":"{parent_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let child_id = serde_json::from_slice::<serde_json::Value>(
        &child_resp.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap()["category_id"]
        .as_str()
        .unwrap()
        .to_string();

    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(format!(
                    r#"{{"url":"{feed_url}","title":"F","category_id":"{child_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/overview")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let cards = json["cards"].as_array().unwrap();
    let child = cards
        .iter()
        .find(|card| card["category_id"] == child_id)
        .expect("child card should exist");
    assert_eq!(
        child["name"], "Top / Sub",
        "overview card shows the full category path, not just the leaf"
    );
}

#[tokio::test]
async fn overview_shows_items_for_categories_with_older_articles() {
    let (_feed_url, app, _server) = spawn_app().await;
    let fresh_srv = feed_server::FeedServer::start(fresh_rss(30), 60);
    let old_srv = feed_server::FeedServer::start(OLD_RSS.to_string(), 60);
    let cookie = cookie_pair(&login_cookie(&app).await);

    let fresh_cat = create_category(&app, &cookie, r#"{"name":"Fresh"}"#).await;
    let old_cat = create_category(&app, &cookie, r#"{"name":"Old"}"#).await;

    add_feed(
        &app,
        &cookie,
        &fresh_srv.url,
        "Fresh Feed",
        Some(&fresh_cat),
    )
    .await;
    add_feed(&app, &cookie, &old_srv.url, "Old Feed", Some(&old_cat)).await;
    refresh_all(&app, &cookie).await;

    let json = overview(&app, &cookie).await;
    let cards = json["cards"].as_array().unwrap();
    let fresh = cards
        .iter()
        .find(|c| c["category_id"] == fresh_cat)
        .expect("fresh card should exist");
    let old = cards
        .iter()
        .find(|c| c["category_id"] == old_cat)
        .expect("old card should exist");

    assert!(
        !fresh["items"].as_array().unwrap().is_empty(),
        "fresh card should show items"
    );
    let old_items = old["items"].as_array().unwrap();
    assert_eq!(
        old_items.len(),
        2,
        "old card must show its own 2 items even though its articles are older than the fresh feed's: {old:?}"
    );
    assert_eq!(old_items[0]["title"], "Old Article");
    assert_eq!(old_items[1]["title"], "Older Article");
}
