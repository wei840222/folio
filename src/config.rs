use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
pub struct Folio {
    pub address: String,
    pub port: u16,
    pub web_path: String,
    pub uploads_path: String,
    pub data_path: String,
    pub garbage_collection_pattern: Vec<String>,
}

impl Folio {
    fn resolve_base(&self, path_str: &str) -> PathBuf {
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

impl Default for Folio {
    fn default() -> Folio {
        Folio {
            address: String::from("127.0.0.1"),
            port: 8000,
            web_path: String::from("./web/dist"),
            uploads_path: String::from("./uploads"),
            data_path: String::from("./data"),
            garbage_collection_pattern: vec![
                String::from(r#"^\._.+"#),
                String::from(r#"^\.DS_Store$"#),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Folio::default();
        assert_eq!(config.address, "127.0.0.1");
        assert_eq!(config.port, 8000);
        assert_eq!(config.web_path, "./web/dist");
        assert_eq!(config.uploads_path, "./uploads");
        assert_eq!(config.garbage_collection_pattern.len(), 2);
    }

    mod build_full_upload_path {
        use super::*;

        #[test]
        fn simple_path() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("test.txt"));

            assert!(path.to_string_lossy().ends_with("uploads/test.txt"));
            assert!(!path.to_string_lossy().contains("/./"));
        }

        #[test]
        fn with_subdirectory() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("subfolder/test.txt"));

            assert!(
                path.to_string_lossy()
                    .ends_with("uploads/subfolder/test.txt")
            );
            assert!(!path.to_string_lossy().contains("/./"));
        }

        #[test]
        fn normalizes_current_dir() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("./test.txt"));

            // Current dir component is ignored, only Normal components remain
            assert!(path.to_string_lossy().ends_with("uploads/test.txt"));
            assert!(!path.to_string_lossy().contains("/./"));
        }

        #[test]
        fn normalizes_parent_dir() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("folder/../test.txt"));

            // Parent dir components are ignored, only Normal components remain
            assert!(path.to_string_lossy().ends_with("uploads/folder/test.txt"));
            assert!(!path.to_string_lossy().contains("/./"));
        }

        #[test]
        fn complex_normalization() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("a/b/../c/./d/../test.txt"));

            // Only Normal components are kept: a, b, c, d, test.txt
            assert!(path.to_string_lossy().ends_with("uploads/a/b/c/d/test.txt"));
        }

        #[test]
        fn with_custom_uploads_path() {
            let config = Folio {
                address: String::from("127.0.0.1"),
                port: 8000,
                web_path: String::from("./web"),
                uploads_path: String::from("./custom_uploads"),
                data_path: String::from("./data"),
                garbage_collection_pattern: vec![],
            };
            let path = config.build_full_upload_path(&PathBuf::from("test.txt"));

            assert!(path.to_string_lossy().ends_with("custom_uploads/test.txt"));
        }

        #[test]
        fn relative_path_uses_current_dir() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("test.txt"));

            // Relative uploads_path should be joined with current_dir
            let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            assert!(path.starts_with(&current_dir));
            assert!(path.to_string_lossy().ends_with("uploads/test.txt"));
        }

        #[test]
        fn absolute_path_ignores_current_dir() {
            let config = Folio {
                address: String::from("127.0.0.1"),
                port: 8000,
                web_path: String::from("./web"),
                uploads_path: String::from("/tmp/test_uploads"),
                data_path: String::from("./data"),
                garbage_collection_pattern: vec![],
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
            let config = Folio::default();

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
            let config = Folio::default();

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
