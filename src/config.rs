use rocket::serde::{Deserialize, Serialize};
use std::path::{Component, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Folio {
    pub web_path: String,
    pub uploads_path: String,
    pub garbage_collection_pattern: Vec<String>,
}

impl Folio {
    /// Build full file path for uploads with normalized path
    pub fn build_full_upload_path(&self, relative_path: &PathBuf) -> PathBuf {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&self.uploads_path);

        // Normalize the relative path to prevent directory traversal
        let normalized = relative_path
            .components()
            .fold(PathBuf::new(), |mut path, component| {
                match component {
                    Component::Normal(c) => path.push(c),
                    _ => {} // Ignore CurDir, ParentDir, Prefix, RootDir
                }
                path
            });

        base.join(normalized)
    }
}

impl<'r> Default for Folio {
    fn default() -> Folio {
        Folio {
            web_path: String::from("./web/dist"),
            uploads_path: String::from("./uploads"),
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
        }

        #[test]
        fn with_subdirectory() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("subfolder/test.txt"));

            assert!(
                path.to_string_lossy()
                    .ends_with("uploads/subfolder/test.txt")
            );
        }

        #[test]
        fn normalizes_current_dir() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("./test.txt"));

            // Current dir component is ignored, only Normal components remain
            assert!(path.to_string_lossy().ends_with("uploads/test.txt"));
        }

        #[test]
        fn normalizes_parent_dir() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("folder/../test.txt"));

            // Parent dir components are ignored, only Normal components remain
            assert!(path.to_string_lossy().ends_with("uploads/folder/test.txt"));
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
                web_path: String::from("./web"),
                uploads_path: String::from("./custom_uploads"),
                garbage_collection_pattern: vec![],
            };
            let path = config.build_full_upload_path(&PathBuf::from("test.txt"));

            assert!(path.to_string_lossy().ends_with("custom_uploads/test.txt"));
        }

        #[test]
        fn absolute_base_path() {
            let config = Folio::default();
            let path = config.build_full_upload_path(&PathBuf::from("test.txt"));

            // Path should start with CARGO_MANIFEST_DIR
            assert!(path.is_absolute() || path.starts_with(env!("CARGO_MANIFEST_DIR")));
        }

        #[test]
        fn prevents_directory_escape() {
            let config = Folio::default();

            // Try to escape with multiple parent directories
            let path = config.build_full_upload_path(&PathBuf::from("../../../etc/passwd"));

            // Path should still contain uploads directory
            assert!(path.to_string_lossy().contains("uploads"));

            // Path should not escape to parent directories
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            assert!(path.starts_with(&manifest_dir) || path.starts_with("uploads"));
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
                let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                assert!(
                    path.starts_with(&manifest_dir)
                        || path.components().any(|c| c.as_os_str() == "uploads"),
                    "Path {} escaped uploads directory",
                    path_str
                );
            }
        }
    }
}
