use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Mutex;

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
    pub fn lock(&self) -> Result<std::sync::MutexGuard<'_, ()>, String> {
        self.lock
            .lock()
            .map_err(|_| "store lock poisoned".to_string())
    }

    /// Load the index from disk. Returns `T::default()` if the file doesn't exist.
    pub fn load(&self) -> Result<T, String> {
        if !self.index_path.exists() {
            return Ok(T::default());
        }
        let raw = std::fs::read_to_string(&self.index_path)
            .map_err(|e| format!("read index failed: {}", e))?;
        serde_json::from_str(&raw).map_err(|e| format!("parse index failed: {}", e))
    }

    /// Atomically save the index to disk (write tmp + rename).
    pub fn save(&self, data: &T) -> Result<(), String> {
        if let Some(parent) = self.index_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create index dir failed: {}", e))?;
        }
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| format!("serialize index failed: {}", e))?;
        let tmp_path = self.index_path.with_extension("json.tmp");
        std::fs::write(&tmp_path, content)
            .map_err(|e| format!("write tmp index failed: {}", e))?;
        std::fs::rename(&tmp_path, &self.index_path)
            .map_err(|e| format!("replace index failed: {}", e))?;
        Ok(())
    }
}
