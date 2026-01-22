// Embedded template management

use rust_embed::RustEmbed;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use thiserror::Error;

/// Error type for template operations
#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("Failed to create temporary directory: {0}")]
    TempDirCreation(#[from] std::io::Error),

    #[error("Failed to extract template file '{path}': {source}")]
    FileExtraction {
        path: String,
        source: std::io::Error,
    },

    #[error("Template file '{0}' not found in embedded templates")]
    TemplateNotFound(String),
}

/// Embedded templates from the templates/ directory
#[derive(RustEmbed)]
#[folder = "templates/"]
pub struct Templates;

/// Extracts all embedded templates to a temporary directory.
///
/// Returns the path to the temporary directory containing all extracted templates.
/// The temporary directory will be cleaned up when the returned `TempDir` is dropped.
///
/// # Errors
///
/// Returns `TemplateError` if:
/// - The temporary directory cannot be created
/// - Any template file cannot be extracted or written
pub fn extract_to_temp() -> Result<(TempDir, PathBuf), TemplateError> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();

    for file_path in Templates::iter() {
        let file_data = Templates::get(&file_path)
            .ok_or_else(|| TemplateError::TemplateNotFound(file_path.to_string()))?;

        let target_path = temp_path.join(file_path.as_ref());

        // Create parent directories if needed
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|e| TemplateError::FileExtraction {
                path: file_path.to_string(),
                source: e,
            })?;
        }

        // Write the file
        fs::write(&target_path, file_data.data.as_ref()).map_err(|e| {
            TemplateError::FileExtraction {
                path: file_path.to_string(),
                source: e,
            }
        })?;
    }

    Ok((temp_dir, temp_path))
}

/// Returns the list of all files that rust-bucket manages in the target repository.
///
/// These are the output filenames (not the template names), representing the files
/// that will be generated or updated by rust-bucket.
pub fn managed_files() -> Vec<&'static str> {
    vec![
        "AGENTS.md",
        "CLAUDE.md",
        "STYLE_GUIDE.md",
        "WORKFLOW.md",
        "WORKFLOW_CODING.md",
        "WORKFLOW_JUDGE.md",
        "WORKFLOW_TIDY.md",
        "WORKFLOW_REFLECTION.md",
        "TESTING.md",
        ".config/nextest.toml",
        "deny.toml",
        "rustfmt.toml",
        ".devcontainer/Dockerfile",
        ".devcontainer/devcontainer.json",
        ".beads/config.yaml",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_to_temp() {
        let result = extract_to_temp();
        assert!(
            result.is_ok(),
            "Failed to extract templates: {:?}",
            result.err()
        );

        let (_temp_dir, temp_path) = result.unwrap();
        assert!(temp_path.exists());
        assert!(temp_path.is_dir());
    }

    #[test]
    fn test_managed_files_not_empty() {
        let files = managed_files();
        assert!(!files.is_empty());
        assert_eq!(files.len(), 15);
    }

    #[test]
    fn test_managed_files_includes_expected() {
        let files = managed_files();
        assert!(files.contains(&"AGENTS.md"));
        assert!(files.contains(&"STYLE_GUIDE.md"));
        assert!(files.contains(&".config/nextest.toml"));
        assert!(files.contains(&".devcontainer/Dockerfile"));
    }
}
