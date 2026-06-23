// Show-migration command implementation — renders embedded migration guides
// for a version range without generating or verifying files.

use crate::config::{Config, ConfigError};
use crate::generator;
use crate::migrations::{self, MigrationError};
use semver::Version;
use std::path::Path;
use thiserror::Error;

/// Outcome of resolving and rendering a migration range.
#[derive(Debug)]
pub enum ShowMigrationOutcome {
    /// Rendered guide text for a non-empty range, ready to print to stdout.
    Guide(String),
    /// The range contained no migrations.
    NoMigrations,
}

/// Errors that can occur while showing migrations.
#[derive(Debug, Error)]
pub enum ShowMigrationError {
    /// `--from` was omitted and no rust-bucket.toml is present to read it from.
    #[error(
        "Not initialized: rust-bucket.toml not found. Pass --from/--to or run inside a rust-bucket repo."
    )]
    NotInitialized,

    /// A version string could not be parsed as full semver.
    #[error("Invalid version '{0}': {1}")]
    VersionParse(String, semver::Error),

    /// The resolved `from` version is greater than the `to` version.
    #[error("Invalid range: 'from' version {0} is greater than 'to' version {1}")]
    FromGreaterThanTo(Version, Version),

    /// Configuration-related error while loading rust-bucket.toml.
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),

    /// Migration lookup error.
    #[error("Migration error: {0}")]
    MigrationError(#[from] MigrationError),
}

/// Resolve a version range and render the migrations within it.
///
/// `to` defaults to this binary's version; `from` defaults to the version
/// recorded in the target directory's rust-bucket.toml. Both bounds must parse
/// as full semver, and `from` must not exceed `to`. An empty range (`from`
/// equal to `to`) is not an error and yields [`ShowMigrationOutcome::NoMigrations`].
pub fn show_migration(
    target_dir: &Path,
    from: Option<String>,
    to: Option<String>,
) -> Result<ShowMigrationOutcome, ShowMigrationError> {
    let to_str = to.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    let from_str = match from {
        Some(value) => value,
        None => {
            if !generator::has_rust_bucket_toml(target_dir) {
                return Err(ShowMigrationError::NotInitialized);
            }
            let config = Config::load(&target_dir.join("rust-bucket.toml"))?;
            config.rust_bucket_version
        }
    };

    let from_version = Version::parse(&from_str)
        .map_err(|e| ShowMigrationError::VersionParse(from_str.clone(), e))?;
    let to_version =
        Version::parse(&to_str).map_err(|e| ShowMigrationError::VersionParse(to_str.clone(), e))?;

    if from_version > to_version {
        return Err(ShowMigrationError::FromGreaterThanTo(
            from_version,
            to_version,
        ));
    }

    let migrations_list = migrations::migrations_between(&from_version, &to_version)?;

    if migrations_list.is_empty() {
        Ok(ShowMigrationOutcome::NoMigrations)
    } else {
        Ok(ShowMigrationOutcome::Guide(migrations::render_migrations(
            &migrations_list,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::TempDir;

    fn write_config(dir: &Path, version: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config = Config {
            rust_bucket_version: version.to_string(),
            test_timeout: 120,
            project_name: "test-project".to_string(),
        };
        config.save(&dir.join("rust-bucket.toml"))?;
        Ok(())
    }

    #[test]
    fn test_default_from_uses_config_version() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        write_config(temp_dir.path(), "0.5.0")?;

        let outcome = show_migration(temp_dir.path(), None, Some("0.7.0".to_string()))?;
        match outcome {
            ShowMigrationOutcome::Guide(text) => {
                assert!(text.contains("# Migration to v0.6.0"));
                assert!(text.contains("# Migration to v0.7.0"));
            }
            ShowMigrationOutcome::NoMigrations => {
                return Err("expected Guide outcome, got NoMigrations".into());
            }
        }
        Ok(())
    }

    #[test]
    fn test_both_flags_work_without_config() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        // No rust-bucket.toml written; explicit flags must still succeed.
        let outcome = show_migration(
            temp_dir.path(),
            Some("0.5.0".to_string()),
            Some("0.6.0".to_string()),
        )?;
        match outcome {
            ShowMigrationOutcome::Guide(text) => {
                assert!(text.contains("# Migration to v0.6.0"));
            }
            ShowMigrationOutcome::NoMigrations => {
                return Err("expected Guide outcome, got NoMigrations".into());
            }
        }
        Ok(())
    }

    #[test]
    fn test_from_equals_to_is_empty_not_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let outcome = show_migration(
            temp_dir.path(),
            Some("0.6.0".to_string()),
            Some("0.6.0".to_string()),
        )?;
        assert!(matches!(outcome, ShowMigrationOutcome::NoMigrations));
        Ok(())
    }

    #[test]
    fn test_from_greater_than_to_is_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let result = show_migration(
            temp_dir.path(),
            Some("0.7.0".to_string()),
            Some("0.6.0".to_string()),
        );
        assert!(matches!(
            result.unwrap_err(),
            ShowMigrationError::FromGreaterThanTo(_, _)
        ));
        Ok(())
    }

    #[test]
    fn test_unparseable_version_is_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let result = show_migration(
            temp_dir.path(),
            Some("v5".to_string()),
            Some("0.7.0".to_string()),
        );
        assert!(matches!(
            result.unwrap_err(),
            ShowMigrationError::VersionParse(_, _)
        ));

        // Partial versions such as "0.9" are also rejected as non-semver.
        let result = show_migration(
            temp_dir.path(),
            Some("0.6.0".to_string()),
            Some("0.9".to_string()),
        );
        assert!(matches!(
            result.unwrap_err(),
            ShowMigrationError::VersionParse(_, _)
        ));
        Ok(())
    }

    #[test]
    fn test_missing_config_without_from_is_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        // No rust-bucket.toml and no --from.
        let result = show_migration(temp_dir.path(), None, Some("0.7.0".to_string()));
        assert!(matches!(
            result.unwrap_err(),
            ShowMigrationError::NotInitialized
        ));
        Ok(())
    }
}
