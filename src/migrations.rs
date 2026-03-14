// Version migration support
//
// Embeds markdown migration files from the migrations/ directory and provides
// a function to retrieve migrations between two versions.

use rust_embed::RustEmbed;
use semver::Version;
use thiserror::Error;

/// Embedded migration files from the migrations/ directory
#[derive(RustEmbed)]
#[folder = "migrations/"]
struct MigrationFiles;

/// A single version migration with instructions
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: Version,
    pub instructions: String,
}

/// Errors that can occur when working with migrations
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("Failed to parse version '{0}': {1}")]
    VersionParse(String, semver::Error),
}

/// Returns all migrations between two versions (exclusive of `from`, inclusive of `to`).
///
/// Migrations are returned sorted by version in ascending order.
/// Filenames that don't parse as semver versions are silently skipped.
///
/// Returns an empty Vec if `from >= to`.
pub fn migrations_between(from: &Version, to: &Version) -> Result<Vec<Migration>, MigrationError> {
    if from >= to {
        return Ok(Vec::new());
    }

    let mut migrations = Vec::new();

    for filename in MigrationFiles::iter() {
        // Strip .md extension to get version string
        let version_str = match filename.strip_suffix(".md") {
            Some(v) => v,
            None => continue,
        };

        // Parse version, skip files that don't parse
        let version = match Version::parse(version_str) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Check if this migration is in range (from < version <= to)
        if version > *from
            && version <= *to
            && let Some(file) = MigrationFiles::get(&filename)
        {
            let instructions = String::from_utf8_lossy(&file.data).to_string();
            migrations.push(Migration {
                version,
                instructions,
            });
        }
    }

    // Sort by version ascending
    migrations.sort_by(|a, b| a.version.cmp(&b.version));

    Ok(migrations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_between_includes_060() {
        let from = Version::new(0, 5, 0);
        let to = Version::new(0, 6, 0);
        let result = migrations_between(&from, &to).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].version, Version::new(0, 6, 0));
    }

    #[test]
    fn test_migrations_between_same_version_returns_empty() {
        let v = Version::new(0, 6, 0);
        let result = migrations_between(&v, &v).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_migrations_between_reversed_returns_empty() {
        let from = Version::new(0, 7, 0);
        let to = Version::new(0, 5, 0);
        let result = migrations_between(&from, &to).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_migration_content_is_non_empty() {
        let from = Version::new(0, 5, 0);
        let to = Version::new(0, 6, 0);
        let result = migrations_between(&from, &to).unwrap();
        assert!(!result.is_empty());
        for migration in &result {
            assert!(!migration.instructions.is_empty());
        }
    }
}
