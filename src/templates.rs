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

/// Returns the .gitignore entries that rust-bucket requires in the target repository.
pub fn required_gitignore_lines() -> Vec<&'static str> {
    vec![
        ".beads/.br_history/",
        ".beads/beads.db",
        ".beads/beads.db-wal",
    ]
}

pub fn managed_files() -> Vec<&'static str> {
    vec![
        "AGENTS.md",
        "CLAUDE.md", // symlink to AGENTS.md, created separately
        "RUST_STYLE_GUIDE.md",
        "TESTING.md",
        ".claude/agents/coordinator.md",
        ".claude/agents/coding.md",
        ".claude/agents/judge.md",
        ".claude/agents/tidy.md",
        ".claude/agents/reflection.md",
        ".config/nextest.toml",
        "deny.toml",
        "rustfmt.toml",
        ".devcontainer/Dockerfile",
        ".devcontainer/devcontainer.json",
        ".beads/config.yaml",
        "justfile-rustbucket",
    ]
}

/// Seed files are written into the target only if absent and are never
/// overwritten on re-apply; the project owns them once present.
///
/// Each entry maps an embedded template path (relative to `templates/`) to its
/// destination path (relative to the target directory). Seed templates must NOT
/// appear in `managed_files()`, and `render()` skips them so they are written
/// only via the seed-if-missing path.
pub fn seed_files() -> Vec<(&'static str, &'static str)> {
    vec![("ratchets.toml.liquid", "ratchets.toml")]
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
        assert_eq!(files.len(), 16);
    }

    #[test]
    fn test_managed_files_includes_expected() {
        let files = managed_files();
        assert!(files.contains(&"AGENTS.md"));
        assert!(files.contains(&"RUST_STYLE_GUIDE.md"));
        assert!(files.contains(&".config/nextest.toml"));
        assert!(files.contains(&".devcontainer/Dockerfile"));
    }

    #[test]
    fn test_seed_files_registers_ratchets_toml() {
        let seeds = seed_files();
        assert!(seeds.contains(&("ratchets.toml.liquid", "ratchets.toml")));
    }

    #[test]
    fn test_ratchets_toml_not_managed() {
        let managed = managed_files();
        assert!(!managed.contains(&"ratchets.toml"));
        assert_eq!(managed.len(), 16);
    }
}
