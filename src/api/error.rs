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
        ApiError {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: message.into(),
        }
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: message.into(),
        }
    }
    pub fn unauthorized(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: message.into(),
        }
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        if let Some(nf) = e.downcast_ref::<news_flash::error::NewsFlashError>() {
            return newsflash_to_api(nf);
        }
        tracing::error!(%e, "request failed");
        ApiError::internal(e.to_string())
    }
}

fn newsflash_to_api(e: &news_flash::error::NewsFlashError) -> ApiError {
    use news_flash::error::NewsFlashError;
    match e {
        NewsFlashError::Syncing => ApiError {
            status: StatusCode::CONFLICT,
            code: "syncing",
            message: "sync in progress".into(),
        },
        NewsFlashError::Offline => ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "offline",
            message: "offline".into(),
        },
        NewsFlashError::Database(err) => {
            if matches!(
                err,
                news_flash::error::DatabaseError::Query(diesel::result::Error::NotFound)
            ) {
                ApiError {
                    status: StatusCode::NOT_FOUND,
                    code: "not_found",
                    message: "not found".into(),
                }
            } else {
                tracing::error!(%e, "internal error");
                ApiError {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    code: "internal",
                    message: "database error".into(),
                }
            }
        }
        NewsFlashError::Thumbnail | NewsFlashError::GrabContent | NewsFlashError::Icon(_) => {
            ApiError {
                status: StatusCode::BAD_GATEWAY,
                code: "upstream",
                message: "content fetch failed".into(),
            }
        }
        NewsFlashError::OPML(_) => ApiError {
            status: StatusCode::BAD_REQUEST,
            code: "bad_opml",
            message: "invalid opml".into(),
        },
        _ => {
            tracing::error!(%e, "internal error");
            ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "internal",
                message: "internal error".into(),
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({ "error": { "code": self.code, "message": self.message } })),
        )
            .into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
