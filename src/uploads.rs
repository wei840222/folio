use std::path::PathBuf;

use rand::Rng;
use rocket::State;
use rocket::form::{Form, Strict};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::{Created, Custom};
use rocket::serde::{Serialize, json::Json};

use super::config;

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

    /// Returns the path to the file in `uploads/` corresponding to this ID.
    pub fn file_path(&self, upload_path: &str, extension: Option<&str>) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(upload_path)
            .join(self.file_name(extension))
    }
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct UploadResponse {
    message: String,
    path: String,
}

#[post("/?<expire>", data = "<file>")]
pub async fn upload_file(
    config: &State<config::Folio>,
    mut file: Form<Strict<TempFile<'_>>>,
    expire: Option<&str>,
) -> Result<Created<Json<UploadResponse>>, Custom<Json<UploadResponse>>> {
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
        let path = candidate.file_path(config.uploads_path.as_str(), ext_ref);
        if !path.exists() {
            break candidate;
        }
    };

    let path = format!("/files/{}", id.file_name(ext_ref));

    // Persist file
    if let Err(e) = file
        .persist_to(id.file_path(config.uploads_path.as_str(), ext_ref))
        .await
    {
        let error_message = format!("failed to save file: {}", e);
        log::error!("{}", error_message);
        return Err(Custom(
            Status::InternalServerError,
            Json(UploadResponse {
                message: error_message,
                path: path,
            }),
        ));
    }

    Ok(Created::new(path.clone()).body(Json(UploadResponse {
        message: format!("file uploaded successfully"),
        path: path,
    })))
}
