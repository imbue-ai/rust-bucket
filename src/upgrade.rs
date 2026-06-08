// Upgrade command implementation — regenerates managed files and collects migrations

use crate::config::{Config, ConfigError};
use crate::generator::{self, GeneratorError};
use crate::migrations::{self, Migration, MigrationError};
use crate::templates::{self, TemplateError};
use crate::verify::{self, VerifyError, VerifyReport};
use semver::Version;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Result of running the upgrade command
#[derive(Debug)]
pub struct UpgradeResult {
    pub old_version: String,
    pub new_version: String,
    pub files_generated: Vec<PathBuf>,
    pub migrations: Vec<Migration>,
    pub verification: VerifyReport,
}

/// Errors that can occur during the upgrade operation
#[derive(Debug, Error)]
pub enum UpgradeError {
    /// Target directory is not a Rust crate (no Cargo.toml found)
    #[error("Not a Rust crate: Cargo.toml not found in target directory")]
    NotRustCrate,

    /// Target directory is not a git repository (no .git/ found)
    #[error("Not a git repository: .git/ directory not found")]
    NotGitRepo,

    /// Target directory is not initialized by rust-bucket (no rust-bucket.toml)
    #[error(
        "Not initialized: rust-bucket.toml not found. Use 'rust-bucket apply' to initialize first."
    )]
    NotInitialized,

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

    /// Migration error
    #[error("Migration error: {0}")]
    MigrationError(#[from] MigrationError),

    /// Version parsing error
    #[error("Invalid version '{0}': {1}")]
    VersionParse(String, semver::Error),
}

/// Run the upgrade command on a target directory.
///
/// Implements the upgrade flow described in ARCHITECTURE.md.
pub fn run_upgrade(target_dir: &Path) -> Result<UpgradeResult, UpgradeError> {
    if !target_dir.join("Cargo.toml").exists() {
        return Err(UpgradeError::NotRustCrate);
    }

    if !target_dir.join(".git").exists() {
        return Err(UpgradeError::NotGitRepo);
    }

    if !generator::has_rust_bucket_toml(target_dir) {
        return Err(UpgradeError::NotInitialized);
    }

    let config_path = target_dir.join("rust-bucket.toml");
    let mut config = Config::load(&config_path)?;
    let old_version_str = config.rust_bucket_version.clone();
    let new_version_str = env!("CARGO_PKG_VERSION").to_string();

    let old_version = Version::parse(&old_version_str)
        .map_err(|e| UpgradeError::VersionParse(old_version_str.clone(), e))?;
    let new_version = Version::parse(&new_version_str)
        .map_err(|e| UpgradeError::VersionParse(new_version_str.clone(), e))?;

    let migrations_list = migrations::migrations_between(&old_version, &new_version)?;

    config.rust_bucket_version = new_version_str.clone();
    config.save(&config_path)?;

    let (_temp_dir, temp_path) = templates::extract_to_temp()?;
    let mut files_generated = generator::render(&temp_path, target_dir, &config, true)?;

    let claude_symlink = generator::create_claude_symlink(target_dir)?;
    files_generated.push(claude_symlink);

    let verification = verify::run_all(target_dir)?;

    Ok(UpgradeResult {
        old_version: old_version_str,
        new_version: new_version_str,
        files_generated,
        migrations: migrations_list,
        verification,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_upgrade_not_rust_crate() {
        let temp_dir = TempDir::new().unwrap();
        let result = run_upgrade(temp_dir.path());
        assert!(matches!(result.unwrap_err(), UpgradeError::NotRustCrate));
    }

    #[test]
    fn test_upgrade_not_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();
        let result = run_upgrade(temp_dir.path());
        assert!(matches!(result.unwrap_err(), UpgradeError::NotGitRepo));
    }

    #[test]
    fn test_upgrade_not_initialized() {
        let temp_dir = TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();
        std::fs::create_dir(temp_dir.path().join(".git")).unwrap();
        let result = run_upgrade(temp_dir.path());
        assert!(matches!(result.unwrap_err(), UpgradeError::NotInitialized));
    }
}
