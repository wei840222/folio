use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;

/// Unified error type for all Agenfact operations.
///
/// Replaces scattered `Result<T, String>` + manual `Custom<Status, Json<...>>` conversions
/// with a single type that knows how to render itself as an HTTP response.
#[derive(Debug)]
pub enum AgenfactError {
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
    RequestTimeout {
        reason: String,
    },
    TooManyRequests {
        reason: String,
        code: &'static str,
        turnstile_site_key: Option<String>,
    },
    InsufficientStorage {
        reason: String,
    },
    Internal {
        source: String,
        context: Option<String>,
    },
}

impl AgenfactError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Forbidden { .. } => StatusCode::FORBIDDEN,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::PayloadTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            Self::RequestTimeout { .. } => StatusCode::REQUEST_TIMEOUT,
            Self::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::InsufficientStorage { .. } => StatusCode::INSUFFICIENT_STORAGE,
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
            Self::RequestTimeout { reason } => reason.clone(),
            Self::TooManyRequests { reason, .. } => reason.clone(),
            Self::InsufficientStorage { reason } => reason.clone(),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    turnstile_site_key: Option<String>,
}

impl std::fmt::Display for AgenfactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl ResponseError for AgenfactError {
    fn status_code(&self) -> StatusCode {
        self.status()
    }

    fn error_response(&self) -> HttpResponse {
        let (code, turnstile_site_key) = match self {
            Self::TooManyRequests {
                code,
                turnstile_site_key,
                ..
            } => (Some(*code), turnstile_site_key.clone()),
            _ => (None, None),
        };
        HttpResponse::build(self.status()).json(ErrorResponse {
            message: self.message(),
            code,
            turnstile_site_key,
        })
    }
}
