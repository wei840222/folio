use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rocket::serde::{Deserialize, Serialize};

use super::config;

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
    index_path: PathBuf,
    lock: Mutex<()>,
}

impl ExpiryStore {
    pub fn new(config: &config::Folio) -> Self {
        let uploads_root = config.build_full_upload_path(&PathBuf::new());
        let index_path = uploads_root.join(".expiry-index.json");

        Self {
            uploads_root,
            index_path,
            lock: Mutex::new(()),
        }
    }

    pub async fn schedule(&self, path: &Path, ttl: Duration) -> Result<(), String> {
        if !path.starts_with(&self.uploads_root) {
            return Err(format!(
                "refuse to schedule path outside uploads root: {}",
                path.display()
            ));
        }

        let _guard = self.lock.lock().await;

        let mut index = self.load_index().await?;

        let normalized = path.to_string_lossy().to_string();
        index.entries.retain(|entry| entry.path != normalized);
        index.entries.push(ExpiryEntry {
            path: normalized,
            expire_at_unix: now_unix_secs().saturating_add(ttl.as_secs()),
        });

        self.save_index(&index).await
    }

    pub fn spawn_sweeper(self: std::sync::Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                if let Err(err) = self.sweep_once().await {
                    log::error!("expiry sweep failed: {}", err);
                }
            }
        });
    }

    async fn sweep_once(&self) -> Result<(), String> {
        let _guard = self.lock.lock().await;

        let mut index = self.load_index().await?;
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

            if tokio::fs::metadata(&target).await.is_ok() {
                match tokio::fs::remove_file(&target).await {
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
        self.save_index(&index).await
    }

    async fn load_index(&self) -> Result<ExpiryIndex, String> {
        match tokio::fs::read_to_string(&self.index_path).await {
            Ok(raw) => {
                serde_json::from_str(&raw).map_err(|e| format!("parse expiry index failed: {}", e))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ExpiryIndex::default()),
            Err(e) => Err(format!("read expiry index failed: {}", e)),
        }
    }

    async fn save_index(&self, index: &ExpiryIndex) -> Result<(), String> {
        if let Some(parent) = self.index_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("create index dir failed: {}", e))?;
        }

        let content = serde_json::to_string_pretty(index)
            .map_err(|e| format!("serialize expiry index failed: {}", e))?;

        let tmp_path = self.index_path.with_extension("json.tmp");
        tokio::fs::write(&tmp_path, content)
            .await
            .map_err(|e| format!("write tmp index failed: {}", e))?;
        tokio::fs::rename(&tmp_path, &self.index_path)
            .await
            .map_err(|e| format!("replace index failed: {}", e))?;

        Ok(())
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
            garbage_collection_pattern: vec![],
        };
        ExpiryStore::new(&config)
    }

    #[tokio::test]
    async fn schedule_writes_expiry_index() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = test_store(&temp_dir);

        let file_path = temp_dir.path().join("hello.txt");
        tokio::fs::write(&file_path, "hello").await.unwrap();

        store
            .schedule(&file_path, Duration::from_secs(120))
            .await
            .unwrap();

        let index_path = temp_dir.path().join(".expiry-index.json");
        assert!(index_path.exists());

        let raw = tokio::fs::read_to_string(index_path).await.unwrap();
        let index: ExpiryIndex = serde_json::from_str(&raw).unwrap();
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].path, file_path.to_string_lossy());
    }

    #[tokio::test]
    async fn sweep_once_deletes_expired_file_and_prunes_entry() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = test_store(&temp_dir);

        let file_path = temp_dir.path().join("expired.txt");
        tokio::fs::write(&file_path, "bye").await.unwrap();

        store.schedule(&file_path, Duration::from_secs(0)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        store.sweep_once().await.unwrap();

        assert!(!file_path.exists());

        let raw = tokio::fs::read_to_string(temp_dir.path().join(".expiry-index.json")).await.unwrap();
        let index: ExpiryIndex = serde_json::from_str(&raw).unwrap();
        assert!(index.entries.is_empty());
    }
}
