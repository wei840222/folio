use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rocket::serde::{Deserialize, Serialize};

use super::config;

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(crate = "rocket::serde")]
struct PrivateIndex {
    files: Vec<String>,
}

pub struct PrivateIndexStore {
    index_path: PathBuf,
    lock: Mutex<()>,
}

impl PrivateIndexStore {
    pub fn new(config: &config::Folio) -> Self {
        let uploads_root = config.build_full_upload_path(&PathBuf::new());
        let index_path = uploads_root.join(".private-files.json");

        Self {
            index_path,
            lock: Mutex::new(()),
        }
    }

    pub fn is_private(&self, relative_path: &Path) -> Result<bool, String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "private index lock poisoned".to_string())?;

        let normalized = relative_path.to_string_lossy().to_string();
        let index = self.load_index()?;
        let set: HashSet<&str> = index.files.iter().map(|s| s.as_str()).collect();

        Ok(set.contains(normalized.as_str()))
    }

    fn load_index(&self) -> Result<PrivateIndex, String> {
        if !self.index_path.exists() {
            return Ok(PrivateIndex::default());
        }

        let raw = std::fs::read_to_string(&self.index_path)
            .map_err(|e| format!("read private index failed: {}", e))?;

        serde_json::from_str(&raw).map_err(|e| format!("parse private index failed: {}", e))
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
            garbage_collection_pattern: vec![],
        };
        PrivateIndexStore::new(&config)
    }

    #[test]
    fn test_is_private_true() {
        let dir = tempdir().unwrap();
        let index_file = dir.path().join(".private-files.json");
        fs::write(index_file, r#"{"files": ["test.txt", "secret.png"]}"#).unwrap();

        let store = setup_store(dir.path());
        assert!(store.is_private(Path::new("test.txt")).unwrap());
        assert!(store.is_private(Path::new("secret.png")).unwrap());
    }

    #[test]
    fn test_is_private_false() {
        let dir = tempdir().unwrap();
        let index_file = dir.path().join(".private-files.json");
        fs::write(index_file, r#"{"files": ["test.txt"]}"#).unwrap();

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
        let index_file = dir.path().join(".private-files.json");
        fs::write(index_file, r#"{"files": []}"#).unwrap();

        let store = setup_store(dir.path());
        assert!(!store.is_private(Path::new("test.txt")).unwrap());
    }

    #[test]
    fn test_is_private_malformed_index() {
        let dir = tempdir().unwrap();
        let index_file = dir.path().join(".private-files.json");
        fs::write(index_file, r#"{"files": ["test.txt"}"#).unwrap(); // missing ]

        let store = setup_store(dir.path());
        assert!(store.is_private(Path::new("test.txt")).is_err());
    }
}
