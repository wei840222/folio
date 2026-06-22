use rocket::http::Status;
use rocket::response::{Responder, Response};
use rocket::serde::Serialize;
use std::io::Cursor;

/// Unified error type for all Folio operations.
///
/// Replaces scattered `Result<T, String>` + manual `Custom<Status, Json<...>>` conversions
/// with a single type that knows how to render itself as an HTTP response.
#[derive(Debug)]
pub enum FolioError {
    NotFound { path: String },
    Forbidden { reason: String },
    Conflict { path: String },
    BadRequest { reason: String },
    Internal { source: String, context: Option<String> },
}

impl FolioError {
    pub fn status(&self) -> Status {
        match self {
            Self::NotFound { .. } => Status::NotFound,
            Self::Forbidden { .. } => Status::Forbidden,
            Self::Conflict { .. } => Status::Conflict,
            Self::BadRequest { .. } => Status::BadRequest,
            Self::Internal { .. } => Status::InternalServerError,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::NotFound { path } => format!("file not found: {}", path),
            Self::Forbidden { reason } => reason.clone(),
            Self::Conflict { path } => format!("file already exists: {}", path),
            Self::BadRequest { reason } => reason.clone(),
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
#[serde(crate = "rocket::serde")]
struct ErrorResponse {
    message: String,
}

impl<'r> Responder<'r, 'static> for FolioError {
    fn respond_to(self, _req: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        let status = self.status();
        let body = serde_json::to_string(&ErrorResponse {
            message: self.message(),
        })
        .unwrap_or_else(|_| r#"{"message":"unknown error"}"#.to_string());

        Response::build()
            .status(status)
            .header(rocket::http::ContentType::JSON)
            .sized_body(body.len(), Cursor::new(body))
            .ok()
    }
}

/// Convert `Result<T, String>` from stores into `Result<T, FolioError>`.
#[allow(dead_code)]
pub trait StoreResultExt<T> {
    fn store_context(self, context: &str) -> Result<T, FolioError>;
}

#[allow(dead_code)]
impl<T> StoreResultExt<T> for Result<T, String> {
    fn store_context(self, context: &str) -> Result<T, FolioError> {
        self.map_err(|e| FolioError::store_error(e, context))
    }
}
