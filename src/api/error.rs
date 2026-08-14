use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn not_found(message: impl Into<String>) -> Self {
        ApiError { status: StatusCode::NOT_FOUND, code: "not_found", message: message.into() }
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError { status: StatusCode::BAD_REQUEST, code: "bad_request", message: message.into() }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        ApiError { status: StatusCode::INTERNAL_SERVER_ERROR, code: "internal", message: message.into() }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(%e, "request failed");
        ApiError::internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": { "code": self.code, "message": self.message } }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
