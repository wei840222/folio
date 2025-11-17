use std::path::PathBuf;

use rand::Rng;
use rocket::State;
use rocket::form::{Form, Strict};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::{Serialize, json::Json};

use super::config;

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct UploadResponse {
    message: String,
    path: String,
}

type UploadResult = Result<Custom<Json<UploadResponse>>, Custom<Json<UploadResponse>>>;

/// Helper to create error response
fn error_response(status: Status, message: String) -> Custom<Json<UploadResponse>> {
    Custom(
        status,
        Json(UploadResponse {
            message,
            path: String::new(),
        }),
    )
}

/// Helper to create success response
fn success_response(message: &str, path: String) -> Custom<Json<UploadResponse>> {
    Custom(
        Status::Created,
        Json(UploadResponse {
            message: message.into(),
            path,
        }),
    )
}

/// A _probably_ unique upload id.
pub struct UploadId(String);

impl UploadId {
    /// Generate a _probably_ unique id with `size` characters. For readability,
    /// the characters used are from the sets [0-9], [A-Z], [a-z]. The
    /// probability of a collision depends on the value of `size` and the number
    /// of ids generated thus far.
    pub fn new(size: usize) -> UploadId {
        const BASE62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

        let mut rng = rand::thread_rng();
        let id: String = (0..size)
            .map(|_| BASE62[rng.gen_range(0..BASE62.len())] as char)
            .collect();

        UploadId(id)
    }

    /// Returns the file name corresponding to this ID.
    pub fn file_name(&self, extension: Option<&str>) -> String {
        extension.map_or_else(|| self.0.clone(), |ext| format!("{}.{}", self.0, ext))
    }
}

#[post("/?<expire>", data = "<file>")]
pub async fn upload_file(
    config: &State<config::Folio>,
    mut file: Form<Strict<TempFile<'_>>>,
    expire: Option<&str>,
) -> UploadResult {
    log::info!("expire: {:?}", expire.unwrap_or("168h"));

    // Determine file extension
    let extension = file
        .content_type()
        .and_then(|ct| ct.extension())
        .map(|s| s.to_string());
    let ext_ref = extension.as_deref();

    // Generate unique ID, retry if file already exists
    let id = loop {
        let candidate = UploadId::new(8);
        let file_name = candidate.file_name(ext_ref);
        let path = config.build_full_upload_path(&PathBuf::from(&file_name));
        if !path.exists() {
            break candidate;
        }
    };

    let file_name = id.file_name(ext_ref);
    let full_path = config.build_full_upload_path(&PathBuf::from(&file_name));
    let response_path = format!("/files/{}", file_name);

    // Persist file
    file.persist_to(&full_path).await.map_err(|e| {
        log::error!("failed to save file: {}", e);
        error_response(
            Status::InternalServerError,
            format!("failed to save file: {}", e),
        )
    })?;

    Ok(success_response(
        "file uploaded successfully",
        response_path,
    ))
}
