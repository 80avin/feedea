use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::AppState;

#[derive(Deserialize)]
pub struct ImgParams {
    pub u: Option<String>,
}

pub async fn proxy_image(Query(params): Query<ImgParams>, State(state): State<AppState>) -> Response {
    let Some(u) = params.u else {
        return (StatusCode::BAD_REQUEST, "missing u").into_response();
    };
    let Ok(parsed) = url::Url::parse(&u) else {
        return (StatusCode::BAD_REQUEST, "invalid url").into_response();
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return (StatusCode::BAD_REQUEST, "only http/https allowed").into_response();
    }
    let client = state.engine.client();
    match client.get(parsed.as_str()).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                return (StatusCode::BAD_GATEWAY, "upstream error").into_response();
            }
            let content_type = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(_) => return (StatusCode::BAD_GATEWAY, "read failed").into_response(),
            };
            ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
        }
        Err(_) => (StatusCode::BAD_GATEWAY, "upstream unavailable").into_response(),
    }
}
