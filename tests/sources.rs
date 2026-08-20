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

async fn spawn_app() -> (
    String,
    axum::Router,
    feed_server::FeedServer,
    Arc<Mutex<app_db::AppDb>>,
) {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let server = feed_server::FeedServer::start(RSS.to_string(), 12);
    let dir = std::env::temp_dir().join(format!(
        "feedea-sources-test-{}-{}",
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
        app_db: app_db.clone(),
        allow_private_proxy: false,
    });
    (server.url.clone(), router, server, app_db)
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

async fn get_groups(app: &axum::Router, cookie: &str) -> Vec<serde_json::Value> {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/sources")
                .header(axum::http::header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    value["groups"].as_array().unwrap().clone()
}

fn find_group<'a>(
    groups: &'a [serde_json::Value],
    category_id: &str,
) -> Option<&'a serde_json::Value> {
    groups.iter().find(|g| g["category_id"] == category_id)
}

fn group_has_feed(group: &serde_json::Value, feed_id: &str) -> bool {
    group["feeds"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == feed_id)
}

fn esc(id: &str) -> String {
    let mut out = String::new();
    for b in id.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[tokio::test]
async fn sources_requires_auth() {
    let (_feed_url, app, _server, _db) = spawn_app().await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/sources")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sources_crud_and_delete_prunes_saved() {
    let (feed_url, app, _server, app_db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(format!(r#"{{"url":"{feed_url}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);
    let body = add_resp.into_body().collect().await.unwrap().to_bytes();
    let feed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let feed_id = feed["id"].as_str().unwrap().to_string();
    assert_eq!(feed["title"], "Test Feed");

    let groups = get_groups(&app, &cookie).await;
    let toplevel =
        find_group(&groups, "NewsFlash.Toplevel").expect("feed should be grouped under toplevel");
    assert!(group_has_feed(toplevel, &feed_id), "feed should be listed");

    let rename_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/sources/{}", esc(&feed_id)))
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(r#"{"title":"Renamed"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rename_resp.status(), StatusCode::OK);

    let groups = get_groups(&app, &cookie).await;
    let toplevel = find_group(&groups, "NewsFlash.Toplevel").unwrap();
    let renamed = toplevel["feeds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == feed_id)
        .unwrap();
    assert_eq!(renamed["title"], "Renamed");

    let read_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sources/{}/read", esc(&feed_id)))
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read_resp.status(), StatusCode::OK);

    let groups = get_groups(&app, &cookie).await;
    let toplevel = find_group(&groups, "NewsFlash.Toplevel").unwrap();
    let read_feed = toplevel["feeds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == feed_id)
        .unwrap();
    assert_eq!(read_feed["unread_count"], 0);

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

    let list_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/articles?offset=0&limit=10")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let body = list_resp.into_body().collect().await.unwrap().to_bytes();
    let articles: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = articles.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let article_id = arr[0]["id"].as_str().unwrap().to_string();

    let save_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/articles/{article_id}/save"))
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(r#"{"note":"must prune"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save_resp.status(), StatusCode::OK);

    let saved_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/saved")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(saved_resp.status(), StatusCode::OK);
    let body = saved_resp.into_body().collect().await.unwrap().to_bytes();
    let saved: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        saved["total"], 1,
        "saved article should be listed before delete"
    );

    let delete_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/sources/{}", esc(&feed_id)))
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), StatusCode::OK);

    let saved_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/saved")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(saved_resp.status(), StatusCode::OK);
    let body = saved_resp.into_body().collect().await.unwrap().to_bytes();
    let saved: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        saved["total"], 0,
        "saved article must be pruned when its feed is deleted"
    );

    let groups = get_groups(&app, &cookie).await;
    assert!(
        groups.iter().all(|g| !group_has_feed(g, &feed_id)),
        "deleted feed must not be listed"
    );

    let db = app_db.lock().await;
    let saved_count: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM saved WHERE article_id = ?1",
            rusqlite::params![article_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(saved_count, 0, "app-db saved row must be gone after delete");
}

#[tokio::test]
async fn sources_can_move_between_categories() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let create_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/categories")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(r#"{"name":"Tech"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let body = create_resp.into_body().collect().await.unwrap().to_bytes();
    let tech_id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["category_id"]
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
                .body(Body::from(format!(r#"{{"url":"{feed_url}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);
    let body = add_resp.into_body().collect().await.unwrap().to_bytes();
    let feed_id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let move_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/sources/{}", esc(&feed_id)))
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(format!(r#"{{"category_id":"{tech_id}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(move_resp.status(), StatusCode::OK);

    let groups = get_groups(&app, &cookie).await;
    let tech = find_group(&groups, &tech_id).expect("feed should be grouped under Tech");
    assert!(
        group_has_feed(tech, &feed_id),
        "feed should appear in Tech group"
    );
    assert!(find_group(&groups, "NewsFlash.Toplevel").is_none_or(|g| !group_has_feed(g, &feed_id)));
}

#[tokio::test]
async fn discover_returns_title_for_feed_url_without_adding() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/discover")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(format!(r#"{{"url":"{feed_url}"}}"#)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["title"], "Test Feed");
    assert_eq!(value["feed_url"], feed_url.as_str());
    assert_eq!(value["alternatives"].as_array().unwrap().len(), 0);

    let groups = get_groups(&app, &cookie).await;
    assert!(
        groups
            .iter()
            .all(|g| g["feeds"].as_array().unwrap().is_empty()),
        "discover must not add a feed"
    );
}

#[tokio::test]
async fn import_and_export_opml() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>My Feeds</title></head>
  <body>
    <outline text="Test Feed" title="Test Feed" type="rss" xmlUrl="{feed_url}"/>
  </body>
</opml>"#
    );
    let body = serde_json::json!({ "opml": opml }).to_string();

    let import_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import_resp.status(), StatusCode::OK);
    let body = import_resp.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["status"], "imported");

    let groups = get_groups(&app, &cookie).await;
    let toplevel = find_group(&groups, "NewsFlash.Toplevel")
        .expect("imported feed should be grouped under toplevel");
    assert!(
        group_has_feed(toplevel, &feed_url),
        "imported feed should be listed"
    );

    let export_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/sources/export-opml")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export_resp.status(), StatusCode::OK);
    assert_eq!(
        export_resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .unwrap(),
        "text/xml"
    );
    let body = export_resp.into_body().collect().await.unwrap().to_bytes();
    let xml = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        xml.contains(&feed_url),
        "exported opml should contain the feed url"
    );
    assert!(!xml.trim().is_empty());
}

#[tokio::test]
async fn opml_exact_duplicate_import_is_skipped() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);
    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Test Feed" title="Test Feed" type="rss" xmlUrl="{feed_url}"/>
  </body>
</opml>"#
    );
    let body = serde_json::json!({ "opml": opml }).to_string();
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_val: serde_json::Value =
        serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(first_val["status"], "imported");
    assert_eq!(first_val["added"], 1);

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_val: serde_json::Value =
        serde_json::from_slice(&second.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(second_val["status"], "imported");
    assert_eq!(second_val["added"], 0);
    assert_eq!(second_val["skipped"], 1);

    let groups = get_groups(&app, &cookie).await;
    let count: usize = groups
        .iter()
        .flat_map(|g| g["feeds"].as_array().unwrap())
        .count();
    assert_eq!(
        count, 1,
        "importing same opml twice must not duplicate the feed"
    );
}

#[tokio::test]
async fn opml_url_variant_conflict_keeps_new_and_migrates_articles() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({ "url": feed_url, "title": "Old Title" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add.status(), StatusCode::OK);

    let variant_url = format!("{feed_url}/");
    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="New Title" title="New Title" type="rss" xmlUrl="{variant_url}"/>
  </body>
</opml>"#
    );

    // phase 1 -> conflicts
    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val["status"], "conflicts");
    let conflict = &val["conflicts"][0];
    assert_eq!(conflict["kind"], "same-feed");
    let key = conflict["key"].as_u64().unwrap() as usize;

    // phase 1 must not write anything: still exactly one feed, unchanged title
    let groups_p1 = get_groups(&app, &cookie).await;
    let feeds_p1: Vec<&serde_json::Value> = groups_p1
        .iter()
        .flat_map(|g| g["feeds"].as_array().unwrap())
        .collect();
    assert_eq!(
        feeds_p1.len(),
        1,
        "conflicts phase must not write any feeds"
    );
    assert_eq!(
        feeds_p1[0]["title"], "Test Feed",
        "conflicts phase must not change the existing feed's title"
    );

    // phase 2 -> keep new
    let resolutions = serde_json::json!([{ "key": key, "action": "keep-new" }]);
    let body2 = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body2))
                .unwrap(),
        )
        .await
        .unwrap();
    let val2: serde_json::Value =
        serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val2["status"], "imported");

    // exactly one feed remains, with the new title, and articles survived the migration
    let groups = get_groups(&app, &cookie).await;
    let feeds: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["feeds"].as_array().unwrap())
        .collect();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0]["title"], "New Title");
    assert_eq!(feeds[0]["feed_url"], variant_url);

    let articles = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/articles")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body_articles = articles.into_body().collect().await.unwrap().to_bytes();
    let val_articles: serde_json::Value = serde_json::from_slice(&body_articles).unwrap();
    let items = val_articles.as_array().unwrap();
    assert_eq!(
        items.len(),
        2,
        "articles from the old feed must be migrated to the new feed"
    );
}

#[tokio::test]
async fn opml_same_url_keep_new_renames_and_moves_to_nested_category() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let create_top = app
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
    assert_eq!(create_top.status(), StatusCode::OK);
    let top_id = serde_json::from_slice::<serde_json::Value>(
        &create_top.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap()["category_id"]
        .as_str()
        .unwrap()
        .to_string();

    let create_sub = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/categories")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(format!(
                    r#"{{"name":"Sub","parent_id":"{top_id}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_sub.status(), StatusCode::OK);
    let sub_id = serde_json::from_slice::<serde_json::Value>(
        &create_sub.into_body().collect().await.unwrap().to_bytes(),
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
                .body(Body::from(
                    serde_json::json!({ "url": feed_url, "title": "Old Title" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    // same raw url as the existing feed, different title and a nested category path
    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Top" title="Top">
      <outline text="Sub" title="Sub">
        <outline text="New Title" title="New Title" type="rss" xmlUrl="{feed_url}"/>
      </outline>
    </outline>
  </body>
</opml>"#
    );

    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val["status"], "conflicts");
    assert_eq!(val["conflicts"][0]["kind"], "same-feed");
    let key = val["conflicts"][0]["key"].as_u64().unwrap() as usize;

    // resolve keep-new: the feed is renamed and moved into the nested category
    let resolutions = serde_json::json!([{ "key": key, "action": "keep-new" }]);
    let body2 = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body2))
                .unwrap(),
        )
        .await
        .unwrap();
    let val2: serde_json::Value =
        serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val2["status"], "imported");

    let groups = get_groups(&app, &cookie).await;
    let feeds: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["feeds"].as_array().unwrap())
        .collect();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0]["title"], "New Title");
    let feed_id = feeds[0]["id"].as_str().unwrap();
    let sub = find_group(&groups, &sub_id).expect("feed should be moved under Top/Sub");
    assert!(group_has_feed(sub, feed_id));
}

#[tokio::test]
async fn opml_import_merges_duplicate_sibling_categories() {
    let (_feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example-a.invalid/feed.xml"/>
    </outline>
    <outline text="Tech" title="Tech">
      <outline text="Feed B" title="Feed B" type="rss" xmlUrl="https://example-b.invalid/feed.xml"/>
    </outline>
  </body>
</opml>"#;
    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val["status"], "imported");
    assert_eq!(val["added"], 2);

    let tree_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/categories")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tree_resp.status(), StatusCode::OK);
    let tree: serde_json::Value =
        serde_json::from_slice(&tree_resp.into_body().collect().await.unwrap().to_bytes()).unwrap();

    let tech: Vec<&serde_json::Value> = tree["categories"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["name"] == "Tech")
        .collect();
    assert_eq!(tech.len(), 1, "exactly one 'Tech' category must exist");

    let groups = get_groups(&app, &cookie).await;
    let feeds: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["feeds"].as_array().unwrap())
        .collect();
    assert_eq!(feeds.len(), 2, "both feeds imported");
}

#[tokio::test]
async fn opml_keep_new_creates_nested_categories_when_nothing_imported() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({ "url": feed_url, "title": "Test Feed" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Top" title="Top">
      <outline text="Sub" title="Sub">
        <outline text="Test Feed" title="Test Feed" type="rss" xmlUrl="{feed_url}"/>
      </outline>
    </outline>
  </body>
</opml>"#
    );
    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val["status"], "conflicts");
    assert_eq!(val["stats"]["new"], 0, "same url means no new feeds");
    let key = val["conflicts"][0]["key"].as_u64().unwrap() as usize;

    let resolutions = serde_json::json!([{ "key": key, "action": "keep-new" }]);
    let body2 = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body2))
                .unwrap(),
        )
        .await
        .unwrap();
    let val2: serde_json::Value =
        serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val2["status"], "imported");
    assert_eq!(val2["added"], 0);
    assert_eq!(val2["updated"], 1, "existing feed reorganized in place");
    assert_eq!(val2["skipped"], 0);
    assert_eq!(val2["conflicts_resolved"], 1);

    let sub_id = sub_category_id(&app, &cookie).await;
    assert!(sub_id.is_some(), "nested categories created on demand");

    let groups = get_groups(&app, &cookie).await;
    let feeds: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["feeds"].as_array().unwrap())
        .collect();
    assert_eq!(feeds.len(), 1);
    assert_eq!(
        feeds[0]["category_id"],
        sub_id.unwrap(),
        "feed moved into Top/Sub"
    );
}

async fn sub_category_id(app: &axum::Router, cookie: &str) -> Option<serde_json::Value> {
    let req = Request::builder()
        .uri("/api/categories")
        .header(axum::http::header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let tv: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let top = tv["categories"]
        .as_array()?
        .iter()
        .find(|c| c["name"] == "Top")?;
    top["children"]
        .as_array()?
        .iter()
        .find(|c| c["name"] == "Sub")?
        .get("category_id")
        .cloned()
}

#[tokio::test]
async fn opml_keep_new_multiple_entries_reuses_created_categories() {
    let (_feed_url, app, _server, _db) = spawn_app().await;
    let server1 = feed_server::FeedServer::start(RSS.to_string(), 12);
    let server2 = feed_server::FeedServer::start(RSS.to_string(), 12);
    let server3 = feed_server::FeedServer::start(RSS.to_string(), 12);
    let cookie = cookie_pair(&login_cookie(&app).await);

    let urls = [
        server1.url.clone(),
        server2.url.clone(),
        server3.url.clone(),
    ];
    for url in &urls {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources")
                    .header("content-type", "application/json")
                    .header(axum::http::header::COOKIE, &cookie)
                    .body(Body::from(
                        serde_json::json!({ "url": url, "title": "Uncategorized" }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Top" title="Top">
      <outline text="A" title="A">
        <outline text="Feed A" title="Feed A" type="rss" xmlUrl="{0}"/>
      </outline>
      <outline text="B" title="B">
        <outline text="Feed B" title="Feed B" type="rss" xmlUrl="{1}"/>
        <outline text="Feed C" title="Feed C" type="rss" xmlUrl="{2}"/>
      </outline>
    </outline>
  </body>
</opml>"#,
        urls[0], urls[1], urls[2]
    );
    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val["status"], "conflicts");
    assert_eq!(val["conflicts"].as_array().unwrap().len(), 3);

    let resolutions: Vec<serde_json::Value> = val["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            serde_json::json!({
                "key": c["key"].as_u64().unwrap() as usize,
                "action": "keep-new",
            })
        })
        .collect();
    let body2 = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body2))
                .unwrap(),
        )
        .await
        .unwrap();
    let val2: serde_json::Value =
        serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val2["status"], "imported");
    assert_eq!(val2["updated"], 3, "all three feeds reorganized");

    let tree_req = Request::builder()
        .uri("/api/categories")
        .header(axum::http::header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let tree_resp = app.clone().oneshot(tree_req).await.unwrap();
    let tv: serde_json::Value =
        serde_json::from_slice(&tree_resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let tops = tv["categories"].as_array().unwrap();
    let top = tops
        .iter()
        .find(|c| c["name"] == "Top")
        .expect("Top created");
    assert_eq!(
        tops.len(),
        1,
        "exactly one Top, no duplicate category for the shared path"
    );
    let children = top["children"].as_array().unwrap();
    let child_a = children
        .iter()
        .find(|c| c["name"] == "A")
        .expect("A created");
    let child_b = children
        .iter()
        .find(|c| c["name"] == "B")
        .expect("B created");
    assert_eq!(children.len(), 2);

    let groups = get_groups(&app, &cookie).await;
    let feeds: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["feeds"].as_array().unwrap())
        .collect();
    assert_eq!(feeds.len(), 3);
    let by_id: std::collections::HashMap<&str, &serde_json::Value> = feeds
        .iter()
        .map(|f| (f["feed_url"].as_str().unwrap(), *f))
        .collect();
    assert_eq!(
        by_id[urls[0].as_str()]["category_id"],
        child_a["category_id"],
        "feed A in Top/A"
    );
    assert_eq!(
        by_id[urls[1].as_str()]["category_id"],
        child_b["category_id"],
        "feed B in Top/B"
    );
    assert_eq!(
        by_id[urls[2].as_str()]["category_id"],
        child_b["category_id"],
        "feed C in Top/B"
    );
    assert_eq!(
        feeds
            .iter()
            .filter(|f| f["title"] == "Uncategorized")
            .count(),
        0,
        "all feeds renamed to the file's titles"
    );
}

#[tokio::test]
async fn sources_list_shows_full_category_path() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({ "url": feed_url, "title": "Test Feed" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    let groups = get_groups(&app, &cookie).await;
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0]["category_name"], "Uncategorized",
        "uncategorized feeds grouped under a readable label, not the toplevel id"
    );

    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Top" title="Top">
      <outline text="Sub" title="Sub">
        <outline text="Test Feed" title="Test Feed" type="rss" xmlUrl="{feed_url}"/>
      </outline>
    </outline>
  </body>
</opml>"#
    );
    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let key = val["conflicts"][0]["key"].as_u64().unwrap() as usize;
    let resolutions = serde_json::json!([{ "key": key, "action": "keep-new" }]);
    let body2 = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body2))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);

    let groups = get_groups(&app, &cookie).await;
    assert_eq!(
        groups.len(),
        1,
        "feed now grouped under its nested category"
    );
    assert_eq!(
        groups[0]["category_name"], "Top / Sub",
        "sources list shows the full category path, not just the leaf"
    );
}

#[tokio::test]
async fn opml_keep_new_moves_to_toplevel_when_file_places_feed_at_root() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({ "url": feed_url, "title": "Test Feed" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    async fn resolve(app: &axum::Router, cookie: &str, opml: &str) -> serde_json::Value {
        let body = serde_json::json!({ "opml": opml }).to_string();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources/import-opml")
                    .header("content-type", "application/json")
                    .header(axum::http::header::COOKIE, cookie)
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let val: serde_json::Value =
            serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
        let key = val["conflicts"][0]["key"].as_u64().unwrap() as usize;
        let resolutions = serde_json::json!([{ "key": key, "action": "keep-new" }]);
        let body2 = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
        let resp2 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sources/import-opml")
                    .header("content-type", "application/json")
                    .header(axum::http::header::COOKIE, cookie)
                    .body(Body::from(body2))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
        serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }

    let categorized = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Test Feed" title="Test Feed" type="rss" xmlUrl="{feed_url}"/>
    </outline>
  </body>
</opml>"#
    );
    let result = resolve(&app, &cookie, &categorized).await;
    assert_eq!(result["updated"], 1, "feed placed into Tech");

    let toplevel = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Test Feed" title="Test Feed" type="rss" xmlUrl="{feed_url}"/>
  </body>
</opml>"#
    );
    let result = resolve(&app, &cookie, &toplevel).await;
    assert_eq!(result["status"], "imported");
    assert_eq!(
        result["updated"], 1,
        "keep-new moved the feed back to toplevel"
    );
    assert_eq!(result["skipped"], 0);

    let groups = get_groups(&app, &cookie).await;
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0]["category_name"], "Uncategorized",
        "feed moved out of Tech to the root"
    );
}

#[tokio::test]
async fn opml_shared_feed_variant_then_identical_both_keep_new_no_error() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({ "url": feed_url, "title": "Original" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    // Same normalized url twice: once as the exact id, once as a trailing-slash
    // variant. Both conflict with the single existing feed.
    let variant_url = format!("{feed_url}/");
    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="File Version" title="File Version" type="rss" xmlUrl="{feed_url}"/>
    <outline text="Variant Version" title="Variant Version" type="rss" xmlUrl="{variant_url}"/>
  </body>
</opml>"#
    );
    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val["status"], "conflicts");
    assert_eq!(val["conflicts"].as_array().unwrap().len(), 2);
    let key_identical = val["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["opml"]["url"] == feed_url)
        .unwrap()["key"]
        .as_u64()
        .unwrap() as usize;
    let key_variant = val["conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["opml"]["url"] == variant_url)
        .unwrap()["key"]
        .as_u64()
        .unwrap() as usize;

    // Resolve the variant FIRST: the old code path would remove the shared feed and
    // then error on the id-identical resolution targeting the gone feed.
    let resolutions = serde_json::json!([
        { "key": key_variant, "action": "keep-new" },
        { "key": key_identical, "action": "keep-new" },
    ]);
    let body2 = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body2))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp2.status(),
        StatusCode::OK,
        "shared-feed resolutions must not 500"
    );
    let val2: serde_json::Value =
        serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val2["status"], "imported");
    assert_eq!(val2["skipped"], 1, "variant keep-new replaced the old feed");
    assert!(val2["migrated"].as_u64().unwrap() > 0, "articles migrated");
    assert_eq!(
        val2["updated"], 0,
        "id-identical resolution saw the feed removed"
    );

    let groups = get_groups(&app, &cookie).await;
    let feeds: Vec<&serde_json::Value> = groups
        .iter()
        .flat_map(|g| g["feeds"].as_array().unwrap())
        .collect();
    assert_eq!(feeds.len(), 1, "replaced by a single variant feed");
    assert_eq!(feeds[0]["feed_url"], variant_url);
}

#[tokio::test]
async fn import_opml_rejects_unknown_resolution_key() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Test Feed" title="Test Feed" type="rss" xmlUrl="{feed_url}"/>
  </body>
</opml>"#
    );
    let resolutions = serde_json::json!([{ "key": 999, "action": "keep-new" }]);
    let body = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn opml_variant_keep_new_lands_in_nested_category() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({ "url": feed_url, "title": "Old" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    let variant_url = format!("{feed_url}/");
    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Top" title="Top">
      <outline text="Sub" title="Sub">
        <outline text="File Version" title="File Version" type="rss" xmlUrl="{variant_url}"/>
      </outline>
    </outline>
  </body>
</opml>"#
    );
    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val["status"], "conflicts");
    let key = val["conflicts"][0]["key"].as_u64().unwrap() as usize;
    let resolutions = serde_json::json!([{ "key": key, "action": "keep-new" }]);
    let body2 = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources/import-opml")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(body2))
                .unwrap(),
        )
        .await
        .unwrap();
    let val2: serde_json::Value =
        serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val2["status"], "imported");
    assert_eq!(
        val2["added"], 1,
        "variant entry imported as the replacement"
    );
    assert_eq!(val2["skipped"], 1, "old feed removed");
    assert!(val2["migrated"].as_u64().unwrap() > 0, "articles migrated");

    let groups = get_groups(&app, &cookie).await;
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0]["category_name"], "Top / Sub",
        "replacement feed lands in the file's nested category, not Uncategorized"
    );
    let feeds = groups[0]["feeds"].as_array().unwrap();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0]["feed_url"], variant_url);
}

#[tokio::test]
async fn delete_empty_categories_removes_only_empty_ones() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let create = |name: &str, parent: Option<&str>| {
        let app = app.clone();
        let cookie = cookie.clone();
        let name = name.to_string();
        let parent = parent.map(|s| s.to_string());
        async move {
            let body = serde_json::json!({ "name": name, "parent_id": parent }).to_string();
            let resp = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/categories")
                        .header("content-type", "application/json")
                        .header(axum::http::header::COOKIE, &cookie)
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            serde_json::from_slice::<serde_json::Value>(
                &resp.into_body().collect().await.unwrap().to_bytes(),
            )
            .unwrap()["category_id"]
                .as_str()
                .unwrap()
                .to_string()
        }
    };

    // Empty toplevel, plus a parent with one empty child.
    let _empty_a = create("Empty A", None).await;
    let _parent = create("Parent", None).await;
    let _empty_child = create("Empty Child", Some(&_parent)).await;
    // A category that will hold the feed (not empty).
    let used = create("Used", None).await;

    // Add the feed directly into `used` so it is not empty.
    let add_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sources")
                .header("content-type", "application/json")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({ "url": feed_url, "title": "Test Feed", "category_id": used })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    let del_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/categories/delete-empty")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del_resp.status(), StatusCode::OK);
    let val: serde_json::Value =
        serde_json::from_slice(&del_resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let deleted = val["deleted"].as_array().unwrap();
    let names: Vec<&str> = deleted.iter().map(|d| d.as_str().unwrap()).collect();
    assert!(
        names.contains(&"Empty A"),
        "empty leaf removed; got {names:?}"
    );
    assert!(
        names.contains(&"Empty Child"),
        "empty child removed; got {names:?}"
    );
    assert!(
        names.contains(&"Parent"),
        "parent left empty after child removal cascades; got {names:?}"
    );
    assert!(!names.contains(&"Used"), "category holding a feed kept");

    // Verify remaining categories.
    let tree_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/categories")
                .header(axum::http::header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let tv: serde_json::Value =
        serde_json::from_slice(&tree_resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let names_left: Vec<&str> = tv["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(names_left.contains(&"Used"));
    assert!(!names_left.contains(&"Empty A"));
    assert!(!names_left.contains(&"Parent"));
    assert!(!names_left.contains(&"Empty Child"));
}
