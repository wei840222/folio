use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::config;
use super::store::JsonFileStore;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrivateEntry {
    pub path: String,
    pub authorized_emails: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
}

pub struct PrivateMutation {
    owner: String,
    previous: Option<PrivateEntry>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct PrivateIndex {
    entries: Vec<PrivateEntry>,
}

pub struct PrivateIndexStore {
    uploads_root: PathBuf,
    store: JsonFileStore<PrivateIndex>,
}

impl PrivateIndexStore {
    pub fn new(config: &config::Agenfact) -> Self {
        let index_path = config.build_full_data_path(&PathBuf::from("private-files.json"));
        Self {
            uploads_root: config.build_full_upload_path(&PathBuf::new()),
            store: JsonFileStore::new(index_path),
        }
    }

    pub async fn reconcile_missing_files(&self) -> Result<usize, String> {
        let _guard = self.store.lock().await?;
        let mut index = self.store.load().await?;
        let original_len = index.entries.len();
        let mut retained = Vec::with_capacity(original_len);
        for entry in index.entries {
            let relative_path = Path::new(&entry.path);
            if !relative_path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
            {
                continue;
            }
            let path = self.uploads_root.join(relative_path);
            match tokio::fs::metadata(&path).await {
                Ok(metadata) if metadata.is_file() => retained.push(entry),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "failed to inspect private-file target {} during startup: {}",
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

    pub async fn mark_private(
        &self,
        relative_path: &Path,
        authorized_emails: Vec<String>,
    ) -> Result<PrivateMutation, String> {
        let _guard = self.store.lock().await?;
        let mut index = self.store.load().await?;
        let normalized = relative_path.to_string_lossy().to_string();
        let owner = format!("{:032x}", rand::random::<u128>());

        let previous = index
            .entries
            .iter()
            .position(|entry| entry.path == normalized)
            .map(|position| index.entries.remove(position));
        index.entries.push(PrivateEntry {
            path: normalized.clone(),
            authorized_emails,
            owner: Some(owner.clone()),
        });

        self.store.save(&index).await?;
        Ok(PrivateMutation { owner, previous })
    }

    pub async fn restore_private(
        &self,
        relative_path: &Path,
        mutation: PrivateMutation,
    ) -> Result<(), String> {
        let _guard = self.store.lock().await?;
        let mut index = self.store.load().await?;
        let normalized = relative_path.to_string_lossy().to_string();
        let current = index
            .entries
            .iter()
            .find(|entry| entry.path == normalized)
            .ok_or_else(|| "private metadata changed before rollback".to_string())?;
        if current.owner.as_deref() != Some(mutation.owner.as_str()) {
            return Err("private metadata changed before rollback".to_string());
        }

        index.entries.retain(|entry| entry.path != normalized);
        if let Some(previous) = mutation.previous {
            index.entries.push(previous);
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
        let _guard = self.store.lock().await?;
        let index = self.store.load().await?;

        Ok(index.entries.iter().any(|e| e.path == normalized))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Agenfact;
    use std::fs;
    use tempfile::tempdir;

    fn setup_store(temp_path: &Path) -> PrivateIndexStore {
        let config = Agenfact {
            address: "127.0.0.1".to_string(),
            port: 8000,
            web_path: "".to_string(),
            uploads_path: temp_path.to_str().unwrap().to_string(),
            data_path: temp_path.to_str().unwrap().to_string(),
            ..Agenfact::default()
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

            let entry = store
                .get_entry(Path::new("test.txt"))
                .await
                .unwrap()
                .unwrap();
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
    fn rollback_restores_only_the_metadata_owned_by_that_upload() {
        let dir = tempdir().unwrap();
        let store = setup_store(dir.path());
        let runtime = rt();

        runtime.block_on(async {
            let path = Path::new("same-id.txt");
            let first = store
                .mark_private(path, vec!["first@example.com".to_string()])
                .await
                .unwrap();
            drop(first);
            let second = store
                .mark_private(path, vec!["second@example.com".to_string()])
                .await
                .unwrap();

            store.restore_private(path, second).await.unwrap();
            assert_eq!(
                store
                    .get_entry(path)
                    .await
                    .unwrap()
                    .unwrap()
                    .authorized_emails,
                vec!["first@example.com"]
            );

            let ours = store
                .mark_private(path, vec!["ours@example.com".to_string()])
                .await
                .unwrap();
            store
                .mark_private(path, vec!["winner@example.com".to_string()])
                .await
                .unwrap();
            assert!(store.restore_private(path, ours).await.is_err());
            assert_eq!(
                store
                    .get_entry(path)
                    .await
                    .unwrap()
                    .unwrap()
                    .authorized_emails,
                vec!["winner@example.com"]
            );
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

    #[test]
    fn startup_reconcile_removes_only_metadata_without_a_final_file() {
        let dir = tempdir().unwrap();
        let store = setup_store(dir.path());
        let runtime = rt();
        runtime.block_on(async {
            store
                .mark_private(
                    Path::new("existing.txt"),
                    vec!["kept@example.com".to_string()],
                )
                .await
                .unwrap();
            store
                .mark_private(
                    Path::new("missing.txt"),
                    vec!["removed@example.com".to_string()],
                )
                .await
                .unwrap();
            fs::write(dir.path().join("existing.txt"), "published").unwrap();

            let removed = store.reconcile_missing_files().await.unwrap();

            assert_eq!(removed, 1);
            assert!(store.is_private(Path::new("existing.txt")).await.unwrap());
            assert!(!store.is_private(Path::new("missing.txt")).await.unwrap());
        });
    }
}
