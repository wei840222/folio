use std::path::{Path, PathBuf};

use rocket::serde::{Deserialize, Serialize};

use super::config;
use super::store::JsonFileStore;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct PrivateEntry {
    pub path: String,
    pub authorized_emails: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(crate = "rocket::serde")]
struct PrivateIndex {
    entries: Vec<PrivateEntry>,
}

pub struct PrivateIndexStore {
    cache: std::sync::RwLock<std::collections::HashMap<String, bool>>,
    store: JsonFileStore<PrivateIndex>,
}

impl PrivateIndexStore {
    pub fn new(config: &config::Folio) -> Self {
        let index_path = config.build_full_data_path(&PathBuf::from("private-files.json"));
        Self {
            cache: std::sync::RwLock::new(std::collections::HashMap::new()),
            store: JsonFileStore::new(index_path),
        }
    }

    pub async fn mark_private(&self, relative_path: &Path, authorized_emails: Vec<String>) -> Result<(), String> {
        let _guard = self.store.lock().await?;
        let mut index = self.store.load().await?;
        let normalized = relative_path.to_string_lossy().to_string();

        index.entries.retain(|e| e.path != normalized);
        index.entries.push(PrivateEntry {
            path: normalized.clone(),
            authorized_emails,
        });

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(normalized, true);
        }

        self.store.save(&index).await
    }

    pub async fn get_entry(&self, relative_path: &Path) -> Result<Option<PrivateEntry>, String> {
        let _guard = self.store.lock().await?;
        let index = self.store.load().await?;
        let normalized = relative_path.to_string_lossy().to_string();

        Ok(index.entries.iter().find(|e| e.path == normalized).cloned())
    }

    pub async fn is_private(&self, relative_path: &Path) -> Result<bool, String> {
        let normalized = relative_path.to_string_lossy().to_string();

        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(&is_priv) = cache.get(&normalized) {
                return Ok(is_priv);
            }
        }

        // Cache miss - load from store
        let _guard = self.store.lock().await?;
        let index = self.store.load().await?;

        let is_priv = index.entries.iter().any(|e| e.path == normalized);

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(normalized, is_priv);
        }

        Ok(is_priv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Folio;
    use std::fs;
    use tempfile::tempdir;

    fn setup_store(temp_path: &Path) -> PrivateIndexStore {
        let config = Folio {
            web_path: "".to_string(),
            uploads_path: temp_path.to_str().unwrap().to_string(),
            data_path: temp_path.to_str().unwrap().to_string(),
            garbage_collection_pattern: vec![],
        };
        PrivateIndexStore::new(&config)
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn test_is_private_true() {
        let dir = tempdir().unwrap();
        let index_file = dir.path().join("private-files.json");
        fs::write(
            &index_file,
            r#"{"entries": [{"path": "test.txt", "authorized_emails": ["a@b.com"]}, {"path": "secret.png", "authorized_emails": []}]}"#,
        )
        .unwrap();

        let store = setup_store(dir.path());
        let runtime = rt();

        runtime.block_on(async {
            assert!(store.is_private(Path::new("test.txt")).await.unwrap());
            assert!(store.is_private(Path::new("secret.png")).await.unwrap());

            let entry = store.get_entry(Path::new("test.txt")).await.unwrap().unwrap();
            assert_eq!(entry.authorized_emails, vec!["a@b.com"]);
        });
    }

    #[test]
    fn test_is_private_false() {
        let dir = tempdir().unwrap();
        let index_file = dir.path().join("private-files.json");
        fs::write(
            &index_file,
            r#"{"entries": [{"path": "test.txt", "authorized_emails": []}]}"#,
        )
        .unwrap();

        let store = setup_store(dir.path());
        let runtime = rt();

        runtime.block_on(async {
            assert!(!store.is_private(Path::new("other.txt")).await.unwrap());
        });
    }

    #[test]
    fn test_is_private_no_index() {
        let dir = tempdir().unwrap();
        let store = setup_store(dir.path());
        let runtime = rt();

        runtime.block_on(async {
            assert!(!store.is_private(Path::new("test.txt")).await.unwrap());
        });
    }

    #[test]
    fn test_is_private_empty_index() {
        let dir = tempdir().unwrap();
        let index_file = dir.path().join("private-files.json");
        fs::write(&index_file, r#"{"entries": []}"#).unwrap();

        let store = setup_store(dir.path());
        let runtime = rt();

        runtime.block_on(async {
            assert!(!store.is_private(Path::new("test.txt")).await.unwrap());
        });
    }

    #[test]
    fn test_is_private_malformed_index() {
        let dir = tempdir().unwrap();
        let index_file = dir.path().join("private-files.json");
        fs::write(&index_file, r#"{"entries": [{"path": "test.txt"}"#).unwrap();

        let store = setup_store(dir.path());
        let runtime = rt();

        runtime.block_on(async {
            assert!(store.is_private(Path::new("test.txt")).await.is_err());
        });
    }
}
