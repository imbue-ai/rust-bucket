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

/// Apply rust-bucket to a target directory for the first time.
///
/// This function implements the first-time flow as specified in ARCHITECTURE.md:
/// 1. Assert Cargo.toml exists (this is a Rust crate)
/// 2. Assert .git/ exists (git init was done)
/// 3. Check for conflicts using generator::check_conflicts()
///    - If conflicts exist and !force, fail with list of conflicts
///    - If conflicts exist and force, warn and continue
/// 4. Prompt for choices (test_timeout) using cli::prompt_test_timeout()
/// 5. Create Config with choices
/// 6. Write rust-bucket.toml using config.save()
/// 7. Extract templates to temp dir using templates::extract_to_temp()
/// 8. Render templates to target dir using generator::render()
/// 9. Run verification using verify::run_all()
/// 10. Return result
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
    // Step 1: Assert Cargo.toml exists
    let cargo_toml = target_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(ApplyError::NotRustCrate);
    }

    // Step 2: Assert .git/ exists
    let git_dir = target_dir.join(".git");
    if !git_dir.exists() {
        return Err(ApplyError::NotGitRepo);
    }

    // Step 3: Check for conflicts
    let conflicts = generator::check_conflicts(target_dir);
    if !conflicts.is_empty() {
        if !force {
            return Err(ApplyError::ConflictingFiles(conflicts));
        }
        // If force is true, we'll continue and overwrite
        eprintln!(
            "Warning: Overwriting {} existing file(s) due to --force flag",
            conflicts.len()
        );
    }

    // Step 4: Prompt for choices
    let test_timeout = cli::prompt_test_timeout()?;

    // Step 5: Create Config with choices
    let config = Config {
        rust_bucket_version: env!("CARGO_PKG_VERSION").to_string(),
        test_timeout,
        project_name: "Rust-Bucket".to_string(),
    };

    // Step 6: Write rust-bucket.toml
    let config_path = target_dir.join("rust-bucket.toml");
    config.save(&config_path)?;

    // Step 7: Extract templates to temp dir
    let (_temp_dir, temp_path) = templates::extract_to_temp()?;

    // Step 8: Render templates to target dir
    // When force is true, we use overwrite=true to replace existing files
    let mut files_generated = generator::render(&temp_path, target_dir, &config, force)?;

    // Step 8b: Create CLAUDE.md symlink to AGENTS.md
    let claude_symlink = generator::create_claude_symlink(target_dir)?;
    files_generated.push(claude_symlink);

    // Step 8c: Ensure .gitignore contains required entries
    generator::ensure_gitignore(target_dir)?;

    // Step 8d: Seed STYLE_GUIDE.md if it doesn't exist
    generator::seed_style_guide(target_dir)?;

    // Step 9: Run verification
    let verification = verify::run_all(target_dir)?;

    // Step 10: Return result
    Ok(ApplyResult {
        files_generated,
        verification,
    })
}

/// Apply rust-bucket to a target directory in update mode (subsequent runs).
///
/// This function implements the subsequent-times flow as specified in ARCHITECTURE.md:
/// 1. Assert Cargo.toml exists (this is a Rust crate)
/// 2. Assert .git/ exists (git init was done)
/// 3. Load rust-bucket.toml using Config::load()
/// 4. Validate config (log/warn if version is different)
/// 5. Pre-populate choices from config (test_timeout already set)
/// 6. Prompt for any NEW choices not in config (none in v1, so skip)
/// 7. Update config's rust_bucket_version to current version
/// 8. Write updated rust-bucket.toml using config.save()
/// 9. Extract templates to temp dir using templates::extract_to_temp()
/// 10. Render templates to target dir using generator::render() with overwrite=true
/// 11. Run verification using verify::run_all()
/// 12. Return result
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
    // Step 1: Assert Cargo.toml exists
    let cargo_toml = target_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(ApplyError::NotRustCrate);
    }

    // Step 2: Assert .git/ exists
    let git_dir = target_dir.join(".git");
    if !git_dir.exists() {
        return Err(ApplyError::NotGitRepo);
    }

    // Step 3: Load rust-bucket.toml
    let config_path = target_dir.join("rust-bucket.toml");
    let mut config = Config::load(&config_path)?;

    // Step 4: Validate config - warn if version is different
    let current_version = env!("CARGO_PKG_VERSION");
    if config.rust_bucket_version != current_version {
        eprintln!(
            "Note: Config was last generated with rust-bucket v{}, updating to v{}",
            config.rust_bucket_version, current_version
        );
    }

    // Step 5: Pre-populate choices from config (test_timeout is already set)
    // The config already contains test_timeout from previous run

    // Step 6: Prompt for any NEW choices not in config
    // In v1, there are no new choices to prompt for, so we skip this step

    // Step 7: Update config's rust_bucket_version to current version
    config.rust_bucket_version = current_version.to_string();

    // Step 8: Write updated rust-bucket.toml
    config.save(&config_path)?;

    // Step 9: Extract templates to temp dir
    let (_temp_dir, temp_path) = templates::extract_to_temp()?;

    // Step 10: Render templates to target dir with overwrite=true
    // Note: The update flow is simpler than init - no conflict checking needed since overwrite=true
    let mut files_generated = generator::render(&temp_path, target_dir, &config, true)?;

    // Step 10b: Create CLAUDE.md symlink to AGENTS.md
    let claude_symlink = generator::create_claude_symlink(target_dir)?;
    files_generated.push(claude_symlink);

    // Step 10c: Ensure .gitignore contains required entries
    generator::ensure_gitignore(target_dir)?;

    // Step 10d: Seed STYLE_GUIDE.md if it doesn't exist
    generator::seed_style_guide(target_dir)?;

    // Step 11: Run verification
    let verification = verify::run_all(target_dir)?;

    // Step 12: Return result
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

    fn create_test_rust_crate(path: &Path) {
        // Create Cargo.toml
        fs::write(
            path.join("Cargo.toml"),
            r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        // Create .git directory
        fs::create_dir(path.join(".git")).unwrap();

        // Create src directory with lib.rs
        let src_dir = path.join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("lib.rs"), "// test lib\n").unwrap();
    }

    #[test]
    fn test_apply_init_not_rust_crate() {
        let temp_dir = TempDir::new().unwrap();
        let result = apply_init(temp_dir.path(), false);

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), ApplyError::NotRustCrate),
            "Expected NotRustCrate error"
        );
    }

    #[test]
    fn test_apply_init_not_git_repo() {
        let temp_dir = TempDir::new().unwrap();

        // Create Cargo.toml but not .git
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();

        let result = apply_init(temp_dir.path(), false);

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), ApplyError::NotGitRepo),
            "Expected NotGitRepo error"
        );
    }

    #[test]
    fn test_apply_init_conflicts_without_force() {
        let temp_dir = TempDir::new().unwrap();
        create_test_rust_crate(temp_dir.path());

        // Create a conflicting file
        fs::write(temp_dir.path().join("AGENTS.md"), "existing content").unwrap();

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
                    .any(|p| p.file_name().unwrap() == "AGENTS.md")
            );
        }
    }
}
