use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rocket::serde::{Deserialize, Serialize};

use super::config;
use super::store::JsonFileStore;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
struct ExpiryEntry {
    path: String,
    expire_at_unix: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(crate = "rocket::serde")]
struct ExpiryIndex {
    entries: Vec<ExpiryEntry>,
}

pub struct ExpiryStore {
    uploads_root: PathBuf,
    store: JsonFileStore<ExpiryIndex>,
}

impl ExpiryStore {
    pub fn new(config: &config::Folio) -> Self {
        let uploads_root = config.build_full_upload_path(&PathBuf::new());
        let index_path = config.build_full_data_path(&PathBuf::from("expiry-index.json"));

        Self {
            uploads_root,
            store: JsonFileStore::new(index_path),
        }
    }

    pub fn schedule(&self, path: &Path, ttl: Duration) -> Result<(), String> {
        if !path.starts_with(&self.uploads_root) {
            return Err(format!(
                "refuse to schedule path outside uploads root: {}",
                path.display()
            ));
        }

        let _guard = self.store.lock()?;
        let mut index = self.store.load()?;

        let normalized = path.to_string_lossy().to_string();
        index.entries.retain(|entry| entry.path != normalized);
        index.entries.push(ExpiryEntry {
            path: normalized,
            expire_at_unix: now_unix_secs().saturating_add(ttl.as_secs()),
        });

        self.store.save(&index)
    }

    pub fn spawn_sweeper(self: std::sync::Arc<Self>, interval: Duration) {
        std::thread::spawn(move || loop {
            std::thread::sleep(interval);
            if let Err(err) = self.sweep_once() {
                log::error!("expiry sweep failed: {}", err);
            }
        });
    }

    fn sweep_once(&self) -> Result<(), String> {
        let _guard = self.store.lock()?;
        let mut index = self.store.load()?;
        let now = now_unix_secs();

        let mut kept = Vec::with_capacity(index.entries.len());
        for entry in index.entries {
            if entry.expire_at_unix > now {
                kept.push(entry);
                continue;
            }

            let target = PathBuf::from(&entry.path);
            if !target.starts_with(&self.uploads_root) {
                log::warn!(
                    "skip deleting out-of-root path from expiry index: {}",
                    entry.path
                );
                continue;
            }

            if target.exists() {
                match std::fs::remove_file(&target) {
                    Ok(_) => log::info!("expired file deleted: {}", target.display()),
                    Err(err) => {
                        log::error!(
                            "failed to delete expired file {}: {}",
                            target.display(),
                            err
                        )
                    }
                }
            }
        }

        index.entries = kept;
        self.store.save(&index)
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store(temp_dir: &tempfile::TempDir) -> ExpiryStore {
        let config = config::Folio {
            web_path: "./web/dist".to_string(),
            uploads_path: temp_dir.path().to_string_lossy().to_string(),
            data_path: temp_dir.path().to_string_lossy().to_string(),
            garbage_collection_pattern: vec![],
        };
        ExpiryStore::new(&config)
    }

    #[test]
    fn schedule_writes_expiry_index() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = test_store(&temp_dir);

        let file_path = temp_dir.path().join("hello.txt");
        std::fs::write(&file_path, "hello").unwrap();

        store
            .schedule(&file_path, Duration::from_secs(120))
            .unwrap();

        let index_path = temp_dir.path().join("expiry-index.json");
        assert!(index_path.exists());

        let raw = std::fs::read_to_string(index_path).unwrap();
        let index: ExpiryIndex = serde_json::from_str(&raw).unwrap();
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].path, file_path.to_string_lossy());
    }

    #[test]
    fn sweep_once_deletes_expired_file_and_prunes_entry() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = test_store(&temp_dir);

        let file_path = temp_dir.path().join("expired.txt");
        std::fs::write(&file_path, "bye").unwrap();

        store.schedule(&file_path, Duration::from_secs(0)).unwrap();
        std::thread::sleep(Duration::from_secs(1));
        store.sweep_once().unwrap();

        assert!(!file_path.exists());

        let raw = std::fs::read_to_string(temp_dir.path().join("expiry-index.json")).unwrap();
        let index: ExpiryIndex = serde_json::from_str(&raw).unwrap();
        assert!(index.entries.is_empty());
    }
}
