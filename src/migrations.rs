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

/// Message shown when a version range contains no migrations.
pub const NO_MIGRATIONS_MESSAGE: &str = "No upgrade instructions";

/// Render a slice of migrations into a single stdout-ready string.
///
/// Each migration's `instructions` are concatenated in ascending version order,
/// separated by a blank line. Callers are responsible for ordering the slice;
/// `migrations_between` already returns migrations sorted ascending.
pub fn render_migrations(migrations: &[Migration]) -> String {
    migrations
        .iter()
        .map(|migration| migration.instructions.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
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
    fn test_migrations_between_includes_060() -> Result<(), Box<dyn std::error::Error>> {
        let from = Version::new(0, 5, 0);
        let to = Version::new(0, 6, 0);
        let result = migrations_between(&from, &to)?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].version, Version::new(0, 6, 0));
        Ok(())
    }

    #[test]
    fn test_migrations_between_same_version_returns_empty() -> Result<(), Box<dyn std::error::Error>>
    {
        let v = Version::new(0, 6, 0);
        let result = migrations_between(&v, &v)?;
        assert!(result.is_empty());
        Ok(())
    }

    #[test]
    fn test_migrations_between_reversed_returns_empty() -> Result<(), Box<dyn std::error::Error>> {
        let from = Version::new(0, 7, 0);
        let to = Version::new(0, 5, 0);
        let result = migrations_between(&from, &to)?;
        assert!(result.is_empty());
        Ok(())
    }

    #[test]
    fn test_migration_content_is_non_empty() -> Result<(), Box<dyn std::error::Error>> {
        let from = Version::new(0, 5, 0);
        let to = Version::new(0, 6, 0);
        let result = migrations_between(&from, &to)?;
        assert!(!result.is_empty());
        for migration in &result {
            assert!(!migration.instructions.is_empty());
        }
        Ok(())
    }

    #[test]
    fn test_render_migrations_orders_ascending_with_blank_line() {
        let migrations = vec![
            Migration {
                version: Version::new(0, 6, 0),
                instructions: "first".to_string(),
            },
            Migration {
                version: Version::new(0, 7, 0),
                instructions: "second".to_string(),
            },
        ];
        assert_eq!(render_migrations(&migrations), "first\n\nsecond");
    }

    #[test]
    fn test_render_migrations_single_has_no_separator() {
        let migrations = vec![Migration {
            version: Version::new(0, 6, 0),
            instructions: "only".to_string(),
        }];
        assert_eq!(render_migrations(&migrations), "only");
    }

    #[test]
    fn test_render_migrations_empty_is_empty_string() {
        assert_eq!(render_migrations(&[]), "");
    }

    #[test]
    fn test_no_migrations_message_constant() {
        assert_eq!(NO_MIGRATIONS_MESSAGE, "No upgrade instructions");
    }
}
