use std::path::{Component, Path, PathBuf};

use super::error::FolioError;

/// A validated, sanitized file path relative to the uploads root.
///
/// This type enforces three invariants at construction time:
/// 1. No path traversal (`..` components rejected)
/// 2. Only `Normal` path components allowed (no `.`, root, or prefix)
/// 3. Can only be constructed through validated paths — no raw `PathBuf` allowed
#[derive(Debug, Clone)]
pub struct SafePath(PathBuf);

impl SafePath {
    /// Create a SafePath from user-provided path segments.
    ///
    /// Rejects `..` components and non-normal components.
    pub fn from_user_input(path: &Path) -> Result<Self, FolioError> {
        // Check for explicit `..` in the string representation
        if path.to_string_lossy().contains("..") {
            log::warn!("path traversal attempt in user input: {}", path.display());
            return Err(FolioError::BadRequest {
                reason: format!("path contains '..': {}", path.to_string_lossy()),
            });
        }

        // Validate all components are Normal
        for component in path.components() {
            match component {
                Component::Normal(_) => {}
                other => {
                    log::warn!(
                        "invalid path component {:?} in user input: {}",
                        other,
                        path.display()
                    );
                    return Err(FolioError::BadRequest {
                        reason: format!("invalid path component in: {}", path.to_string_lossy()),
                    });
                }
            }
        }

        Ok(SafePath(path.to_path_buf()))
    }

    /// Get the inner Path reference.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for SafePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
