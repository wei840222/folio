use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;

/// Unified error type for all Folio operations.
///
/// Replaces scattered `Result<T, String>` + manual `Custom<Status, Json<...>>` conversions
/// with a single type that knows how to render itself as an HTTP response.
#[derive(Debug)]
pub enum FolioError {
    Unauthorized {
        reason: String,
    },
    NotFound {
        path: String,
    },
    Forbidden {
        reason: String,
    },
    Conflict {
        path: String,
    },
    BadRequest {
        reason: String,
    },
    PayloadTooLarge {
        reason: String,
    },
    Internal {
        source: String,
        context: Option<String>,
    },
}

impl FolioError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Forbidden { .. } => StatusCode::FORBIDDEN,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::PayloadTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Unauthorized { reason } => reason.clone(),
            Self::NotFound { path } => format!("file not found: {}", path),
            Self::Forbidden { reason } => reason.clone(),
            Self::Conflict { path } => format!("file already exists: {}", path),
            Self::BadRequest { reason } => reason.clone(),
            Self::PayloadTooLarge { reason } => reason.clone(),
            Self::Internal { source, context } => match context {
                Some(ctx) => format!("{}: {}", ctx, source),
                None => source.clone(),
            },
        }
    }

    /// Convert a `Result<T, String>` from a store into an internal error with context.
    pub fn store_error(source: String, context: &str) -> Self {
        Self::Internal {
            source,
            context: Some(context.to_string()),
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

impl std::fmt::Display for FolioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl ResponseError for FolioError {
    fn status_code(&self) -> StatusCode {
        self.status()
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status()).json(ErrorResponse {
            message: self.message(),
        })
    }
}
