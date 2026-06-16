// Apply command implementation for first-time and subsequent runs

use crate::cli;
use crate::config::{Config, ConfigError};
use crate::generator::{self, GeneratorError};
use crate::templates::{self, TemplateError};
use crate::verify::{self, VerifyError, VerifyReport};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Result of applying rust-bucket to a target directory
#[derive(Debug)]
pub struct ApplyResult {
    pub files_generated: Vec<PathBuf>,
    pub verification: VerifyReport,
}

/// Errors that can occur during the apply operation
#[derive(Debug, Error)]
pub enum ApplyError {
    /// Target directory is not a Rust crate (no Cargo.toml found)
    #[error("Not a Rust crate: Cargo.toml not found in target directory")]
    NotRustCrate,

    /// Target directory is not a git repository (no .git/ found)
    #[error("Not a git repository: .git/ directory not found")]
    NotGitRepo,

    /// Conflicting files exist in the target directory
    #[error("Conflicting files detected: {}", .0.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))]
    ConflictingFiles(Vec<PathBuf>),

    /// Configuration-related error
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),

    /// Template generation error
    #[error("Generator error: {0}")]
    GeneratorError(#[from] GeneratorError),

    /// Verification error
    #[error("Verification error: {0}")]
    VerifyError(#[from] VerifyError),

    /// Template extraction error
    #[error("Template error: {0}")]
    TemplateError(#[from] TemplateError),

    /// CLI interaction error
    #[error("CLI error: {0}")]
    CliError(#[from] cli::CliError),
}

/// Derive the project name for a target repository.
///
/// Prefers the `[package].name` declared in the target's `Cargo.toml`. Falls
/// back to the target directory's file name when the manifest has no package
/// name (e.g. a virtual workspace root) or cannot be parsed.
fn derive_project_name(target_dir: &Path) -> String {
    let cargo_toml = target_dir.join("Cargo.toml");
    if let Ok(contents) = std::fs::read_to_string(&cargo_toml)
        && let Ok(value) = contents.parse::<toml::Value>()
        && let Some(name) = value
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(|name| name.as_str())
    {
        return name.to_string();
    }

    target_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_string())
}

/// Apply rust-bucket to a target directory for the first time.
///
/// Implements the first-time flow described in ARCHITECTURE.md.
///
/// # Arguments
///
/// * `target_dir` - The target directory to apply rust-bucket to
/// * `force` - If true, overwrite existing managed files; if false, fail on conflicts
///
/// # Errors
///
/// Returns `ApplyError` if:
/// - The target is not a Rust crate (no Cargo.toml)
/// - The target is not a git repository (no .git/)
/// - Conflicting files exist and force is false
/// - Any step in the process fails (config save, template extraction, rendering, verification)
pub fn apply_init(target_dir: &Path, force: bool) -> Result<ApplyResult, ApplyError> {
    let cargo_toml = target_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(ApplyError::NotRustCrate);
    }

    let git_dir = target_dir.join(".git");
    if !git_dir.exists() {
        return Err(ApplyError::NotGitRepo);
    }

    let conflicts = generator::check_conflicts(target_dir);
    if !conflicts.is_empty() {
        if !force {
            return Err(ApplyError::ConflictingFiles(conflicts));
        }
        eprintln!(
            "Warning: Overwriting {} existing file(s) due to --force flag",
            conflicts.len()
        );
    }

    let test_timeout = cli::prompt_test_timeout()?;

    let config = Config {
        rust_bucket_version: env!("CARGO_PKG_VERSION").to_string(),
        test_timeout,
        project_name: derive_project_name(target_dir),
    };

    let config_path = target_dir.join("rust-bucket.toml");
    config.save(&config_path)?;

    let (_temp_dir, temp_path) = templates::extract_to_temp()?;

    let mut files_generated = generator::render(&temp_path, target_dir, &config, force)?;

    let claude_symlink = generator::create_claude_symlink(target_dir)?;
    files_generated.push(claude_symlink);

    generator::ensure_gitignore(target_dir)?;

    let seeded = generator::seed_files(&temp_path, target_dir, &config)?;
    files_generated.extend(seeded);

    let verification = verify::run_all(target_dir)?;

    Ok(ApplyResult {
        files_generated,
        verification,
    })
}

/// Apply rust-bucket to a target directory in update mode (subsequent runs).
///
/// Implements the update flow described in ARCHITECTURE.md.
///
/// # Arguments
///
/// * `target_dir` - The target directory to update rust-bucket files in
///
/// # Errors
///
/// Returns `ApplyError` if:
/// - The target is not a Rust crate (no Cargo.toml)
/// - The target is not a git repository (no .git/)
/// - The rust-bucket.toml config file cannot be loaded
/// - Any step in the process fails (config save, template extraction, rendering, verification)
pub fn apply_update(target_dir: &Path) -> Result<ApplyResult, ApplyError> {
    let cargo_toml = target_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(ApplyError::NotRustCrate);
    }

    let git_dir = target_dir.join(".git");
    if !git_dir.exists() {
        return Err(ApplyError::NotGitRepo);
    }

    let config_path = target_dir.join("rust-bucket.toml");
    let mut config = Config::load(&config_path)?;

    let current_version = env!("CARGO_PKG_VERSION");
    if config.rust_bucket_version != current_version {
        eprintln!(
            "Note: Config was last generated with rust-bucket v{}, updating to v{}",
            config.rust_bucket_version, current_version
        );
    }

    config.rust_bucket_version = current_version.to_string();

    config.save(&config_path)?;

    let (_temp_dir, temp_path) = templates::extract_to_temp()?;

    let mut files_generated = generator::render(&temp_path, target_dir, &config, true)?;

    let claude_symlink = generator::create_claude_symlink(target_dir)?;
    files_generated.push(claude_symlink);

    generator::ensure_gitignore(target_dir)?;

    let seeded = generator::seed_files(&temp_path, target_dir, &config)?;
    files_generated.extend(seeded);

    let verification = verify::run_all(target_dir)?;

    Ok(ApplyResult {
        files_generated,
        verification,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_rust_crate(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Create Cargo.toml
        fs::write(
            path.join("Cargo.toml"),
            r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"
"#,
        )?;

        // Create .git directory
        fs::create_dir(path.join(".git"))?;

        // Create src directory with lib.rs
        let src_dir = path.join("src");
        fs::create_dir(&src_dir)?;
        fs::write(src_dir.join("lib.rs"), "// test lib\n")?;
        Ok(())
    }

    #[test]
    fn test_derive_project_name_from_cargo_toml() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        create_test_rust_crate(temp_dir.path())?;

        assert_eq!(derive_project_name(temp_dir.path()), "test-crate");
        Ok(())
    }

    #[test]
    fn test_derive_project_name_falls_back_to_dir_name() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let workspace_root = temp_dir.path().join("my-workspace");
        fs::create_dir(&workspace_root)?;

        // Workspace manifest without a [package] section.
        fs::write(
            workspace_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate-a\"]\n",
        )?;

        assert_eq!(derive_project_name(&workspace_root), "my-workspace");
        Ok(())
    }

    #[test]
    fn test_apply_init_not_rust_crate() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let result = apply_init(temp_dir.path(), false);

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), ApplyError::NotRustCrate),
            "Expected NotRustCrate error"
        );
        Ok(())
    }

    #[test]
    fn test_apply_init_not_git_repo() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        // Create Cargo.toml but not .git
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )?;

        let result = apply_init(temp_dir.path(), false);

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), ApplyError::NotGitRepo),
            "Expected NotGitRepo error"
        );
        Ok(())
    }

    #[test]
    fn test_apply_init_conflicts_without_force() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        create_test_rust_crate(temp_dir.path())?;

        // Create a conflicting file
        fs::write(temp_dir.path().join("AGENTS.md"), "existing content")?;

        let result = apply_init(temp_dir.path(), false);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ApplyError::ConflictingFiles(_)),
            "Expected ConflictingFiles error"
        );
        if let ApplyError::ConflictingFiles(conflicts) = err {
            assert!(!conflicts.is_empty());
            assert!(
                conflicts
                    .iter()
                    .any(|p| p.file_name().is_some_and(|n| n == "AGENTS.md"))
            );
        }
        Ok(())
    }
}
