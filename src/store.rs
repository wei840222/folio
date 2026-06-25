use std::marker::PhantomData;
use std::path::PathBuf;
use tokio::sync::Mutex;

/// Generic JSON file store with atomic writes and mutex protection.
///
/// Provides `load`/`save` for any `T: Serialize + DeserializeOwned + Default`,
/// plus a `lock()` method for callers that need to perform read-modify-write
/// operations atomically.
pub struct JsonFileStore<T> {
    index_path: PathBuf,
    lock: Mutex<()>,
    _marker: PhantomData<T>,
}

impl<T> JsonFileStore<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Default,
{
    pub fn new(index_path: PathBuf) -> Self {
        Self {
            index_path,
            lock: Mutex::new(()),
            _marker: PhantomData,
        }
    }

    /// Acquire the store mutex.
    pub async fn lock(&self) -> Result<tokio::sync::MutexGuard<'_, ()>, String> {
        Ok(self.lock.lock().await)
    }

    /// Load the index from disk. Returns `T::default()` if the file doesn't exist.
    pub async fn load(&self) -> Result<T, String> {
        if !self.index_path.exists() {
            return Ok(T::default());
        }
        let raw = tokio::fs::read_to_string(&self.index_path)
            .await
            .map_err(|e| format!("read index failed: {}", e))?;
        serde_json::from_str(&raw).map_err(|e| format!("parse index failed: {}", e))
    }

    /// Atomically save the index to disk (write tmp + rename).
    pub async fn save(&self, data: &T) -> Result<(), String> {
        if let Some(parent) = self.index_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("create index dir failed: {}", e))?;
        }
        let content =
            serde_json::to_string(data).map_err(|e| format!("serialize index failed: {}", e))?;
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
