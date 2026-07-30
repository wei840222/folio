use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::config;
use super::store::JsonFileStore;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExpiryEntry {
    path: String,
    expire_at_unix: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
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

    pub async fn publish(
        &self,
        staged_path: &Path,
        final_path: &Path,
        ttl: Duration,
    ) -> Result<u64, String> {
        let file_name = final_path
            .file_name()
            .ok_or_else(|| format!("publish path has no file name: {}", final_path.display()))?;
        let final_parent = final_path
            .parent()
            .ok_or_else(|| format!("publish path has no parent: {}", final_path.display()))?;
        let final_path = tokio::fs::canonicalize(final_parent)
            .await
            .map_err(|error| {
                format!(
                    "failed to resolve publish parent {}: {}",
                    final_parent.display(),
                    error
                )
            })?
            .join(file_name);
        if !final_path.starts_with(&self.uploads_root) {
            return Err(format!(
                "refuse to publish path outside uploads root: {}",
                final_path.display()
            ));
        }

        let expire_at_unix = now_unix_secs()
            .checked_add(ttl.as_secs())
            .filter(|expires_at| *expires_at <= config::MAX_CLIENT_EXPIRY_UNIX_SECS)
            .ok_or_else(|| {
                format!(
                    "upload expiration would exceed client expiration limit ({})",
                    config::MAX_CLIENT_EXPIRY_UNIX_SECS
                )
            })?;
        let _guard = self.store.lock().await?;
        let mut index = self.store.load().await?;
        let normalized = final_path.to_string_lossy().to_string();
        let previous_entries: Vec<_> = index
            .entries
            .iter()
            .filter(|entry| entry.path == normalized)
            .cloned()
            .collect();
        index.entries.retain(|entry| entry.path != normalized);
        index.entries.push(ExpiryEntry {
            path: normalized.clone(),
            expire_at_unix,
        });
        self.store.save(&index).await?;

        if let Err(error) = tokio::fs::hard_link(staged_path, &final_path).await {
            index.entries.retain(|entry| entry.path != normalized);
            index.entries.extend(previous_entries);
            if let Err(rollback_error) = self.store.save(&index).await {
                return Err(format!(
                    "publish failed: {}; expiry rollback failed: {}",
                    error, rollback_error
                ));
            }
            return Err(format!("publish failed: {}", error));
        }

        if let Err(error) = tokio::fs::remove_file(staged_path).await {
            log::warn!(
                "published {} but failed to remove staging link {}: {}",
                final_path.display(),
                staged_path.display(),
                error
            );
        }

        Ok(expire_at_unix)
    }

    pub async fn reconcile_missing_files(&self) -> Result<usize, String> {
        let _guard = self.store.lock().await?;
        let mut index = self.store.load().await?;
        let original_len = index.entries.len();
        let mut retained = Vec::with_capacity(original_len);
        for entry in index.entries {
            let path = PathBuf::from(&entry.path);
            if !path.starts_with(&self.uploads_root) {
                continue;
            }
            match tokio::fs::metadata(&path).await {
                Ok(metadata) if metadata.is_file() => retained.push(entry),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "failed to inspect expiry target {} during startup: {}",
                        path.display(),
                        error
                    ));
                }
            }
        }
        let removed = original_len - retained.len();
        if removed > 0 {
            index.entries = retained;
            self.store.save(&index).await?;
        }
        Ok(removed)
    }

    pub fn spawn_sweeper(self: std::sync::Arc<Self>, interval: Duration) {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            loop {
                std::thread::sleep(interval);
                if let Err(err) = rt.block_on(self.sweep_once()) {
                    log::error!("expiry sweep failed: {}", err);
                }
            }
        });
    }

    async fn sweep_once(&self) -> Result<(), String> {
        let _guard = self.store.lock().await?;
        let mut index = self.store.load().await?;
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
        self.store.save(&index).await
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
            address: "127.0.0.1".to_string(),
            port: 8000,
            web_path: "./web/dist".to_string(),
            uploads_path: temp_dir.path().to_string_lossy().to_string(),
            data_path: temp_dir.path().to_string_lossy().to_string(),
            max_upload_size: 25 * 1024 * 1024,
            default_upload_ttl_secs: 7 * 24 * 60 * 60,
            max_upload_ttl_secs: 7 * 24 * 60 * 60,
            max_upload_text_field_size: 4 * 1024,
            max_authorized_emails: 50,
        };
        ExpiryStore::new(&config)
    }

    #[tokio::test]
    async fn publish_registers_expiry_before_making_file_visible() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = test_store(&temp_dir);
        let staging_dir = temp_dir.path().join(config::UPLOAD_STAGING_DIR);
        std::fs::create_dir_all(&staging_dir).unwrap();
        let staged_path = staging_dir.join("pending.part");
        let final_path = temp_dir.path().join("published.txt");
        std::fs::write(&staged_path, "content").unwrap();

        store
            .publish(&staged_path, &final_path, Duration::from_secs(0))
            .await
            .unwrap();

        assert!(!staged_path.exists());
        assert!(final_path.exists());
        store.sweep_once().await.unwrap();
        assert!(!final_path.exists());

        let raw = std::fs::read_to_string(temp_dir.path().join("expiry-index.json")).unwrap();
        let index: ExpiryIndex = serde_json::from_str(&raw).unwrap();
        assert!(index.entries.is_empty());
    }

    #[tokio::test]
    async fn publish_never_overwrites_an_existing_final_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = test_store(&temp_dir);
        let staging_dir = temp_dir.path().join(config::UPLOAD_STAGING_DIR);
        std::fs::create_dir_all(&staging_dir).unwrap();
        let staged_path = staging_dir.join("pending.part");
        let final_path = temp_dir.path().join("existing.txt");
        std::fs::write(&staged_path, "replacement").unwrap();
        std::fs::write(&final_path, "original").unwrap();
        let original_entry = ExpiryEntry {
            path: final_path.to_string_lossy().to_string(),
            expire_at_unix: 123_456,
        };
        store
            .store
            .save(&ExpiryIndex {
                entries: vec![original_entry.clone()],
            })
            .await
            .unwrap();

        let result = store
            .publish(&staged_path, &final_path, Duration::from_secs(120))
            .await;

        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&final_path).unwrap(), "original");
        assert_eq!(
            std::fs::read_to_string(&staged_path).unwrap(),
            "replacement"
        );
        let raw = std::fs::read_to_string(temp_dir.path().join("expiry-index.json")).unwrap();
        let index: ExpiryIndex = serde_json::from_str(&raw).unwrap();
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].path, original_entry.path);
        assert_eq!(
            index.entries[0].expire_at_unix,
            original_entry.expire_at_unix
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn publish_accepts_an_uploads_root_symlink() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().unwrap();
        let real_uploads = temp_dir.path().join("real-uploads");
        let uploads_link = temp_dir.path().join("uploads-link");
        std::fs::create_dir(&real_uploads).unwrap();
        symlink(&real_uploads, &uploads_link).unwrap();
        let config = config::Folio {
            uploads_path: uploads_link.to_string_lossy().to_string(),
            data_path: temp_dir.path().to_string_lossy().to_string(),
            ..config::Folio::default()
        };
        let store = ExpiryStore::new(&config);
        let staging_dir = uploads_link.join(config::UPLOAD_STAGING_DIR);
        std::fs::create_dir(&staging_dir).unwrap();
        let staged_path = staging_dir.join("pending.part");
        let final_path = uploads_link.join("published.txt");
        std::fs::write(&staged_path, "content").unwrap();

        store
            .publish(&staged_path, &final_path, Duration::from_secs(120))
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(final_path).unwrap(), "content");
    }

    #[tokio::test]
    async fn startup_reconcile_removes_only_metadata_without_a_final_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = test_store(&temp_dir);
        let existing = temp_dir.path().join("existing.txt");
        let missing = temp_dir.path().join("missing.txt");
        std::fs::write(&existing, "published").unwrap();
        store
            .store
            .save(&ExpiryIndex {
                entries: vec![
                    ExpiryEntry {
                        path: existing.to_string_lossy().to_string(),
                        expire_at_unix: u64::MAX,
                    },
                    ExpiryEntry {
                        path: missing.to_string_lossy().to_string(),
                        expire_at_unix: u64::MAX,
                    },
                ],
            })
            .await
            .unwrap();

        let removed = store.reconcile_missing_files().await.unwrap();

        assert_eq!(removed, 1);
        let index = store.store.load().await.unwrap();
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].path, existing.to_string_lossy());
    }

    #[tokio::test]
    async fn publish_rejects_expiration_beyond_client_range() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = test_store(&temp_dir);
        let staging_dir = temp_dir.path().join(config::UPLOAD_STAGING_DIR);
        std::fs::create_dir_all(&staging_dir).unwrap();
        let staged_path = staging_dir.join("pending.part");
        let final_path = temp_dir.path().join("published.txt");
        std::fs::write(&staged_path, "content").unwrap();

        let error = store
            .publish(
                &staged_path,
                &final_path,
                Duration::from_secs(config::MAX_CLIENT_EXPIRY_UNIX_SECS),
            )
            .await
            .unwrap_err();

        assert!(error.contains("client expiration limit"));
        assert!(staged_path.exists());
        assert!(!final_path.exists());
        assert!(store.store.load().await.unwrap().entries.is_empty());
    }
}
