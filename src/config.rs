use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const UPLOAD_STAGING_DIR: &str = ".agenfact-staging";
pub const MAX_CLIENT_EXPIRY_UNIX_SECS: u64 = 253_402_300_799;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Agenfact {
    pub address: String,
    pub port: u16,
    pub web_path: String,
    pub uploads_path: String,
    pub data_path: String,
    pub max_upload_size: usize,
    pub default_upload_ttl_secs: u64,
    pub max_upload_ttl_secs: u64,
    pub max_upload_text_field_size: usize,
    pub max_authorized_emails: usize,
    pub couchdb_url: Option<String>,
    pub couchdb_db: String,
    pub couchdb_user: Option<String>,
    pub couchdb_password: Option<String>,
}

impl Agenfact {
    pub fn validate(&self) -> Result<(), String> {
        let now_unix_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "system clock is before the Unix epoch".to_string())?
            .as_secs();
        self.validate_at(now_unix_secs)
    }

    fn validate_at(&self, now_unix_secs: u64) -> Result<(), String> {
        if self.max_upload_ttl_secs == 0 {
            return Err("maximum upload TTL must be greater than zero".to_string());
        }
        if self.default_upload_ttl_secs == 0 {
            return Err("default upload TTL must be greater than zero".to_string());
        }
        if self.default_upload_ttl_secs > self.max_upload_ttl_secs {
            return Err(format!(
                "default upload TTL ({} seconds) exceeds maximum ({} seconds)",
                self.default_upload_ttl_secs, self.max_upload_ttl_secs
            ));
        }
        if now_unix_secs
            .checked_add(self.max_upload_ttl_secs)
            .is_none_or(|expires_at| expires_at > MAX_CLIENT_EXPIRY_UNIX_SECS)
        {
            return Err(format!(
                "maximum upload TTL would exceed client expiration limit ({MAX_CLIENT_EXPIRY_UNIX_SECS})"
            ));
        }
        Ok(())
    }

    pub fn resolve_base(&self, path_str: &str) -> PathBuf {
        let p = PathBuf::from(path_str);
        if p.is_absolute() {
            p
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path_str)
        }
    }

    /// Build full file path for uploads with normalized path
    pub fn build_full_upload_path(&self, relative_path: &Path) -> PathBuf {
        self.normalize_and_join(&self.resolve_base(&self.uploads_path), relative_path)
    }

    pub fn build_upload_staging_path(&self, relative_path: &Path) -> PathBuf {
        let staging_root = self
            .resolve_base(&self.uploads_path)
            .join(UPLOAD_STAGING_DIR);
        self.normalize_and_join(&staging_root, relative_path)
    }

    /// Build full file path for persistent data
    pub fn build_full_data_path(&self, relative_path: &Path) -> PathBuf {
        self.normalize_and_join(&self.resolve_base(&self.data_path), relative_path)
    }

    fn normalize_and_join(&self, base: &Path, relative_path: &Path) -> PathBuf {
        // Normalize the relative path to prevent directory traversal
        let normalized = relative_path
            .components()
            .fold(PathBuf::new(), |mut path, component| {
                if let Component::Normal(c) = component {
                    path.push(c);
                }
                path
            });

        let full_path = base.join(normalized);

        // Only call canonicalize() when path exists — it's an expensive syscall
        // (resolves symlinks, hits the filesystem). For new uploads, the path
        // won't exist yet so skip straight to the cheap fallback.
        if full_path.exists()
            && let Ok(p) = full_path.canonicalize()
        {
            return p;
        }

        // Fallback: manually clean up CurDir (.) components
        full_path.components().fold(PathBuf::new(), |mut p, c| {
            match c {
                Component::CurDir => {}
                _ => p.push(c),
            }
            p
        })
    }
}

impl Default for Agenfact {
    fn default() -> Agenfact {
        Agenfact {
            address: String::from("127.0.0.1"),
            port: 8000,
            web_path: String::from("./web/dist"),
            uploads_path: String::from("./uploads"),
            data_path: String::from("./data"),
            max_upload_size: 25 * 1024 * 1024, // 25 MiB
            default_upload_ttl_secs: 7 * 24 * 60 * 60,
            max_upload_ttl_secs: 7 * 24 * 60 * 60,
            max_upload_text_field_size: 4 * 1024,
            max_authorized_emails: 50,
            couchdb_url: None,
            couchdb_db: String::from("agenfact"),
            couchdb_user: None,
            couchdb_password: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Agenfact::default();
        assert_eq!(config.address, "127.0.0.1");
        assert_eq!(config.port, 8000);
        assert_eq!(config.web_path, "./web/dist");
        assert_eq!(config.uploads_path, "./uploads");
        assert_eq!(config.max_upload_size, 25 * 1024 * 1024);
        assert_eq!(config.default_upload_ttl_secs, 7 * 24 * 60 * 60);
        assert_eq!(config.max_upload_ttl_secs, 7 * 24 * 60 * 60);
        assert_eq!(config.max_upload_text_field_size, 4 * 1024);
        assert_eq!(config.max_authorized_emails, 50);
    }

    #[test]
    fn rejects_default_upload_ttl_above_maximum() {
        let config = Agenfact {
            default_upload_ttl_secs: 121,
            max_upload_ttl_secs: 120,
            ..Agenfact::default()
        };

        assert_eq!(
            config.validate(),
            Err("default upload TTL (121 seconds) exceeds maximum (120 seconds)".to_string())
        );
    }

    #[test]
    fn rejects_zero_default_upload_ttl() {
        let config = Agenfact {
            default_upload_ttl_secs: 0,
            max_upload_ttl_secs: 1,
            ..Agenfact::default()
        };

        assert_eq!(
            config.validate_at(1_000),
            Err("default upload TTL must be greater than zero".to_string())
        );
    }

    #[test]
    fn rejects_zero_maximum_upload_ttl() {
        let config = Agenfact {
            default_upload_ttl_secs: 0,
            max_upload_ttl_secs: 0,
            ..Agenfact::default()
        };

        assert_eq!(
            config.validate_at(1_000),
            Err("maximum upload TTL must be greater than zero".to_string())
        );
    }

    #[test]
    fn accepts_maximum_ttl_that_reaches_client_expiration_limit() {
        let config = Agenfact {
            default_upload_ttl_secs: 100,
            max_upload_ttl_secs: 100,
            ..Agenfact::default()
        };

        assert_eq!(
            config.validate_at(MAX_CLIENT_EXPIRY_UNIX_SECS - 100),
            Ok(())
        );
    }

    #[test]
    fn rejects_maximum_ttl_beyond_client_expiration_limit() {
        let config = Agenfact {
            default_upload_ttl_secs: 100,
            max_upload_ttl_secs: 101,
            ..Agenfact::default()
        };

        assert_eq!(
            config.validate_at(MAX_CLIENT_EXPIRY_UNIX_SECS - 100),
            Err(format!(
                "maximum upload TTL would exceed client expiration limit ({MAX_CLIENT_EXPIRY_UNIX_SECS})"
            ))
        );
    }

    mod build_full_upload_path {
        use super::*;

        #[test]
        fn simple_path() {
            let config = Agenfact::default();
            let path = config.build_full_upload_path(&PathBuf::from("test.txt"));

            assert!(path.to_string_lossy().ends_with("uploads/test.txt"));
            assert!(!path.to_string_lossy().contains("/./"));
        }

        #[test]
        fn with_subdirectory() {
            let config = Agenfact::default();
            let path = config.build_full_upload_path(&PathBuf::from("subfolder/test.txt"));

            assert!(
                path.to_string_lossy()
                    .ends_with("uploads/subfolder/test.txt")
            );
            assert!(!path.to_string_lossy().contains("/./"));
        }

        #[test]
        fn normalizes_current_dir() {
            let config = Agenfact::default();
            let path = config.build_full_upload_path(&PathBuf::from("./test.txt"));

            // Current dir component is ignored, only Normal components remain
            assert!(path.to_string_lossy().ends_with("uploads/test.txt"));
            assert!(!path.to_string_lossy().contains("/./"));
        }

        #[test]
        fn normalizes_parent_dir() {
            let config = Agenfact::default();
            let path = config.build_full_upload_path(&PathBuf::from("folder/../test.txt"));

            // Parent dir components are ignored, only Normal components remain
            assert!(path.to_string_lossy().ends_with("uploads/folder/test.txt"));
            assert!(!path.to_string_lossy().contains("/./"));
        }

        #[test]
        fn complex_normalization() {
            let config = Agenfact::default();
            let path = config.build_full_upload_path(&PathBuf::from("a/b/../c/./d/../test.txt"));

            // Only Normal components are kept: a, b, c, d, test.txt
            assert!(path.to_string_lossy().ends_with("uploads/a/b/c/d/test.txt"));
        }

        #[test]
        fn with_custom_uploads_path() {
            let config = Agenfact {
                uploads_path: String::from("./custom_uploads"),
                ..Agenfact::default()
            };
            let path = config.build_full_upload_path(&PathBuf::from("test.txt"));

            assert!(path.to_string_lossy().ends_with("custom_uploads/test.txt"));
        }

        #[test]
        fn relative_path_uses_current_dir() {
            let config = Agenfact::default();
            let path = config.build_full_upload_path(&PathBuf::from("test.txt"));

            // Relative uploads_path should be joined with current_dir
            let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            assert!(path.starts_with(&current_dir));
            assert!(path.to_string_lossy().ends_with("uploads/test.txt"));
        }

        #[test]
        fn absolute_path_ignores_current_dir() {
            let config = Agenfact {
                uploads_path: String::from("/tmp/test_uploads"),
                ..Agenfact::default()
            };
            let path = config.build_full_upload_path(&PathBuf::from("test.txt"));

            // Absolute uploads_path should be used directly
            assert_eq!(path, PathBuf::from("/tmp/test_uploads/test.txt"));

            // Should NOT contain current_dir
            let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            assert!(
                !path.starts_with(&current_dir) || !path.to_string_lossy().contains("test_uploads")
            );
        }

        #[test]
        fn prevents_directory_escape() {
            let config = Agenfact::default();

            // Try to escape with multiple parent directories
            let path = config.build_full_upload_path(&PathBuf::from("../../../etc/passwd"));

            // Path should still contain uploads directory
            assert!(path.to_string_lossy().contains("uploads"));

            // Path should not escape to parent directories
            let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            assert!(path.starts_with(&current_dir) || path.starts_with("uploads"));
        }

        #[test]
        fn prevents_escape_with_dots() {
            let config = Agenfact::default();

            // Multiple attempts to escape
            let test_cases = vec![
                "../../../../etc/passwd",
                "../secret.txt",
                "folder/../../outside.txt",
                "./../../sensitive.dat",
            ];

            for test_path in test_cases {
                let path = config.build_full_upload_path(&PathBuf::from(test_path));
                let path_str = path.to_string_lossy();

                // Should not contain ../ after normalization
                assert!(
                    !path_str.contains("/../"),
                    "Path {} contains /../",
                    path_str
                );

                // Should still be within project directory
                let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                assert!(
                    path.starts_with(&current_dir)
                        || path.components().any(|c| c.as_os_str() == "uploads"),
                    "Path {} escaped uploads directory",
                    path_str
                );
            }
        }
    }
}
