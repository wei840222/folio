use std::sync::Arc;

use actix_web::web::Bytes;
use futures_util::Stream;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

use super::config::Agenfact;
use super::error::AgenfactError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CouchDoc {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    pub path: String,
    pub size_bytes: u64,
    pub content_type: String,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authorized_emails: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_at_unix: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PutResponse {
    pub rev: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ViewRow {
    pub id: String,
    pub doc: Option<CouchDoc>,
}

#[derive(Debug, Deserialize)]
struct ViewResponse {
    pub rows: Vec<ViewRow>,
}

#[derive(Clone)]
pub struct CouchDbStore {
    client: Client,
    db_url: String,
}

impl CouchDbStore {
    pub fn new(config: &Agenfact) -> Option<Self> {
        let couch_url = config.couchdb_url.as_ref()?;
        let base_url = couch_url.trim_end_matches('/');
        let db_url = format!("{}/{}", base_url, urlencoding_simple(&config.couchdb_db));

        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            HeaderValue::from_static("Agenfact-Backend/1.0"),
        );

        if let (Some(user), Some(pass)) = (&config.couchdb_user, &config.couchdb_password) {
            let auth = format!("{}:{}", user, pass);
            let encoded = base64_simple(&auth);
            if let Ok(hv) = HeaderValue::from_str(&format!("Basic {}", encoded)) {
                headers.insert(reqwest::header::AUTHORIZATION, hv);
            }
        }

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .ok()?;

        Some(Self { client, db_url })
    }

    pub async fn init_db(&self) -> Result<(), AgenfactError> {
        // Ensure Database
        let res = self
            .client
            .get(&self.db_url)
            .send()
            .await
            .map_err(|e| AgenfactError::Internal {
                source: format!("Failed to reach CouchDB: {}", e),
                context: Some("init_db check".into()),
            })?;

        if res.status() == StatusCode::NOT_FOUND {
            let create_res = self
                .client
                .put(&self.db_url)
                .send()
                .await
                .map_err(|e| AgenfactError::Internal {
                    source: format!("Failed to create CouchDB database: {}", e),
                    context: Some("init_db create".into()),
                })?;
            if !create_res.status().is_success() {
                return Err(AgenfactError::Internal {
                    source: format!("CouchDB database creation HTTP {}", create_res.status()),
                    context: Some("init_db create response".into()),
                });
            }
        }

        // Ensure Expiry Design Document
        let view_url = format!("{}/_design/expiry", self.db_url);
        let view_res = self.client.get(&view_url).send().await;
        if let Ok(resp) = view_res
            && resp.status() == StatusCode::NOT_FOUND
        {
            let design_doc = serde_json::json!({
                "_id": "_design/expiry",
                "views": {
                    "by_expire_at": {
                        "map": "function (doc) { if (doc.expire_at_unix && doc.expire_at_unix > 0) { emit(doc.expire_at_unix, null); } }"
                    }
                }
            });
            let _ = self.client.put(&view_url).json(&design_doc).send().await;
        }

        Ok(())
    }

    pub async fn get_doc(&self, id: &str) -> Result<Option<CouchDoc>, AgenfactError> {
        let url = format!("{}/{}", self.db_url, urlencoding_simple(id));
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AgenfactError::Internal {
                source: format!("CouchDB GET doc error: {}", e),
                context: Some(format!("get_doc {}", id)),
            })?;

        if res.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !res.status().is_success() {
            return Err(AgenfactError::Internal {
                source: format!("CouchDB GET doc status {}", res.status()),
                context: Some(format!("get_doc {}", id)),
            });
        }

        let doc = res
            .json::<CouchDoc>()
            .await
            .map_err(|e| AgenfactError::Internal {
                source: format!("CouchDB json parse error: {}", e),
                context: Some(format!("get_doc {}", id)),
            })?;

        Ok(Some(doc))
    }

    pub async fn put_doc(&self, mut doc: CouchDoc) -> Result<CouchDoc, AgenfactError> {
        let url = format!("{}/{}", self.db_url, urlencoding_simple(&doc.id));

        // If rev is not set, check if doc already exists to get rev
        if doc.rev.is_none()
            && let Ok(Some(existing)) = self.get_doc(&doc.id).await
        {
            doc.rev = existing.rev;
        }

        let res = self
            .client
            .put(&url)
            .json(&doc)
            .send()
            .await
            .map_err(|e| AgenfactError::Internal {
                source: format!("CouchDB PUT doc error: {}", e),
                context: Some(format!("put_doc {}", doc.id)),
            })?;

        if !res.status().is_success() {
            return Err(AgenfactError::Internal {
                source: format!("CouchDB PUT doc status {}", res.status()),
                context: Some(format!("put_doc {}", doc.id)),
            });
        }

        let put_resp = res.json::<PutResponse>().await.ok();
        if let Some(resp) = put_resp {
            doc.rev = resp.rev;
        }

        Ok(doc)
    }

    pub async fn put_attachment<S>(&self, id: &str, rev: &str, content_type: &str, stream: S) -> Result<String, AgenfactError>
    where
        S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + Sync + 'static,
    {
        let url = format!(
            "{}/{}/file?rev={}",
            self.db_url,
            urlencoding_simple(id),
            urlencoding_simple(rev)
        );

        let res = self
            .client
            .put(&url)
            .header(CONTENT_TYPE, content_type)
            .body(reqwest::Body::wrap_stream(stream))
            .send()
            .await
            .map_err(|e| AgenfactError::Internal {
                source: format!("CouchDB PUT attachment error: {}", e),
                context: Some(format!("put_attachment {}", id)),
            })?;

        if !res.status().is_success() {
            return Err(AgenfactError::Internal {
                source: format!("CouchDB PUT attachment status {}", res.status()),
                context: Some(format!("put_attachment {}", id)),
            });
        }

        let put_resp = res.json::<PutResponse>().await.ok();
        let new_rev = put_resp
            .and_then(|r| r.rev)
            .unwrap_or_else(|| rev.to_string());

        Ok(new_rev)
    }

    pub async fn get_attachment_stream(
        &self,
        id: &str,
    ) -> Result<Option<(String, u64, impl Stream<Item = Result<Bytes, reqwest::Error>>)>, AgenfactError>
    {
        let doc = self.get_doc(id).await?;
        let doc = match doc {
            Some(d) => d,
            None => return Ok(None),
        };

        let url = format!("{}/{}/file", self.db_url, urlencoding_simple(id));
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AgenfactError::Internal {
                source: format!("CouchDB GET attachment error: {}", e),
                context: Some(format!("get_attachment {}", id)),
            })?;

        if res.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !res.status().is_success() {
            return Err(AgenfactError::Internal {
                source: format!("CouchDB GET attachment status {}", res.status()),
                context: Some(format!("get_attachment {}", id)),
            });
        }

        let content_type = doc.content_type;
        let content_length = doc.size_bytes;
        let stream = res.bytes_stream();

        Ok(Some((content_type, content_length, stream)))
    }

    pub async fn delete_doc(&self, id: &str, rev: &str) -> Result<bool, AgenfactError> {
        let url = format!(
            "{}/{}?rev={}",
            self.db_url,
            urlencoding_simple(id),
            urlencoding_simple(rev)
        );
        let res = self.client.delete(&url).send().await;
        match res {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    pub async fn get_expired_docs(&self, now_unix: u64) -> Result<Vec<CouchDoc>, AgenfactError> {
        let url = format!(
            "{}/_design/expiry/_view/by_expire_at?endkey={}&include_docs=true",
            self.db_url, now_unix
        );

        let res = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AgenfactError::Internal {
                source: format!("CouchDB GET expired view error: {}", e),
                context: Some("get_expired_docs".into()),
            })?;

        if !res.status().is_success() {
            return Ok(Vec::new());
        }

        let view_resp = res.json::<ViewResponse>().await.ok();
        let docs = view_resp
            .map(|vr| vr.rows.into_iter().filter_map(|r| r.doc).collect())
            .unwrap_or_default();

        Ok(docs)
    }

    pub fn spawn_sweeper(self: Arc<Self>, interval: std::time::Duration) {
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    log::error!("failed to build runtime for CouchDB sweeper: {}", e);
                    return;
                }
            };
            rt.block_on(async move {
                let mut timer = tokio::time::interval(interval);
                loop {
                    timer.tick().await;
                    let now_unix = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                        Ok(d) => d.as_secs(),
                        Err(_) => continue,
                    };
                    match self.get_expired_docs(now_unix).await {
                        Ok(expired_docs) => {
                            for doc in expired_docs {
                                if let Some(rev) = &doc.rev {
                                    log::info!("CouchDB sweeper removing expired doc: {}", doc.id);
                                    let _ = self.delete_doc(&doc.id, rev).await;
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("CouchDB expiry sweep failed: {}", e);
                        }
                    }
                }
            });
        });
    }
}

fn urlencoding_simple(s: &str) -> String {
    s.split('/')
        .map(|seg| {
            seg.chars()
                .map(|c| match c {
                    'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                    _ => format!("%{:02X}", c as u8),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn base64_simple(input: &str) -> String {
    use std::fmt::Write;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };

        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        let c0 = CHARS[((n >> 18) & 63) as usize] as char;
        let c1 = CHARS[((n >> 12) & 63) as usize] as char;
        let c2 = if i + 1 < bytes.len() {
            CHARS[((n >> 6) & 63) as usize] as char
        } else {
            '='
        };
        let c3 = if i + 2 < bytes.len() {
            CHARS[(n & 63) as usize] as char
        } else {
            '='
        };

        let _ = write!(out, "{}{}{}{}", c0, c1, c2, c3);
        i += 3;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Agenfact;

    #[test]
    fn test_urlencoding_simple() {
        assert_eq!(urlencoding_simple("hello/world"), "hello/world");
        assert_eq!(urlencoding_simple("a b/c@d"), "a%20b/c%40d");
        assert_eq!(urlencoding_simple("agenfact_db"), "agenfact_db");
    }

    #[test]
    fn test_base64_simple() {
        assert_eq!(base64_simple("admin:password"), "YWRtaW46cGFzc3dvcmQ=");
        assert_eq!(base64_simple("foo"), "Zm9v");
        assert_eq!(base64_simple("foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_couchdb_store_new_without_url() {
        let config = Agenfact::default();
        assert!(CouchDbStore::new(&config).is_none());
    }

    #[test]
    fn test_couchdb_store_new_with_url() {
        let config = Agenfact {
            couchdb_url: Some("http://localhost:5984".to_string()),
            couchdb_db: "agenfact".to_string(),
            couchdb_user: Some("admin".to_string()),
            couchdb_password: Some("password".to_string()),
            ..Agenfact::default()
        };
        let store = CouchDbStore::new(&config);
        assert!(store.is_some());
        let store = store.unwrap();
        assert_eq!(store.db_url, "http://localhost:5984/agenfact");
    }

    #[test]
    fn test_couch_doc_serde() {
        let doc = CouchDoc {
            id: "file123".to_string(),
            rev: Some("1-abc".to_string()),
            path: "report.pdf".to_string(),
            size_bytes: 1024,
            content_type: "application/pdf".to_string(),
            is_private: true,
            authorized_emails: vec!["user@example.com".to_string()],
            expire_at_unix: Some(1_700_000_000),
            owner: Some("owner1".to_string()),
        };
        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("\"_id\":\"file123\""));
        assert!(json.contains("\"_rev\":\"1-abc\""));
        assert!(json.contains("\"is_private\":true"));

        let deserialized: CouchDoc = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "file123");
        assert_eq!(deserialized.authorized_emails, vec!["user@example.com"]);
    }
}

