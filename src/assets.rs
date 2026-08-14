use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "frontend/dist"]
pub struct Assets;

fn index_html() -> Response {
    match Assets::get("index.html") {
        Some(file) => ([(header::CONTENT_TYPE, "text/html")], file.data.into_owned()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn fallback(uri: Uri) -> Response {
    let path = uri.path();
    if path.starts_with("/api/") || path.starts_with("/img") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return index_html();
    }
    if let Some(file) = Assets::get(trimmed) {
        let mime = mime_guess::from_path(trimmed).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref().to_string())], file.data.into_owned())
            .into_response();
    }
    if !last_segment_has_extension(trimmed) {
        return index_html();
    }
    StatusCode::NOT_FOUND.into_response()
}

fn last_segment_has_extension(path: &str) -> bool {
    path.rsplit('/').next().is_some_and(|seg| seg.contains('.'))
}
