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
    store: JsonFileStore<PrivateIndex>,
}

impl PrivateIndexStore {
    pub fn new(config: &config::Folio) -> Self {
        let index_path = config.build_full_data_path(&PathBuf::from("private-files.json"));
        Self {
            store: JsonFileStore::new(index_path),
        }
    }

    pub fn mark_private(&self, relative_path: &Path, authorized_emails: Vec<String>) -> Result<(), String> {
        let _guard = self.store.lock()?;

        let mut index = self.store.load()?;
        let normalized = relative_path.to_string_lossy().to_string();

        index.entries.retain(|e| e.path != normalized);
        index.entries.push(PrivateEntry {
            path: normalized,
            authorized_emails,
        });

        self.store.save(&index)
    }

    pub fn get_entry(&self, relative_path: &Path) -> Result<Option<PrivateEntry>, String> {
        let _guard = self.store.lock()?;

        let normalized = relative_path.to_string_lossy().to_string();
        let index = self.store.load()?;

        Ok(index.entries.iter().find(|e| e.path == normalized).cloned())
    }

    pub fn is_private(&self, relative_path: &Path) -> Result<bool, String> {
        Ok(self.get_entry(relative_path)?.is_some())
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
        assert!(store.is_private(Path::new("test.txt")).unwrap());
        assert!(store.is_private(Path::new("secret.png")).unwrap());

        let entry = store.get_entry(Path::new("test.txt")).unwrap().unwrap();
        assert_eq!(entry.authorized_emails, vec!["a@b.com"]);
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
        assert!(!store.is_private(Path::new("other.txt")).unwrap());
    }

    #[test]
    fn test_is_private_no_index() {
        let dir = tempdir().unwrap();
        let store = setup_store(dir.path());
        assert!(!store.is_private(Path::new("test.txt")).unwrap());
    }

    #[test]
    fn test_is_private_empty_index() {
        let dir = tempdir().unwrap();
        let index_file = dir.path().join("private-files.json");
        fs::write(&index_file, r#"{"entries": []}"#).unwrap();

        let store = setup_store(dir.path());
        assert!(!store.is_private(Path::new("test.txt")).unwrap());
    }

    #[test]
    fn test_is_private_malformed_index() {
        let dir = tempdir().unwrap();
        let index_file = dir.path().join("private-files.json");
        fs::write(&index_file, r#"{"entries": [{"path": "test.txt"}"#).unwrap();

        let store = setup_store(dir.path());
        assert!(store.is_private(Path::new("test.txt")).is_err());
    }
}
