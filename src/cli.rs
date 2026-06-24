// CLI argument parsing and command dispatch

use clap::{Parser, Subcommand};
use std::io::{self, BufRead, Write};
use thiserror::Error;

/// Rust-first project bootstrapper for AI-first engineering
#[derive(Debug, Parser)]
#[command(name = "rust-bucket")]
#[command(about = "Rust-first project bootstrapper for AI-first engineering")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Apply rust-bucket to the current directory
    Apply {
        /// Force overwrite of existing managed files
        #[arg(long)]
        force: bool,
    },
    /// Show embedded migration guides for a version range
    ShowMigration {
        /// Version to migrate from (defaults to the current rust-bucket.toml version)
        #[arg(long)]
        from: Option<String>,
        /// Version to migrate to (defaults to this binary's version)
        #[arg(long)]
        to: Option<String>,
    },
}

/// CLI-related errors
#[derive(Debug, Error)]
pub enum CliError {
    /// IO error during interactive prompting
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Invalid input provided by the user
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Prompt the user for a test timeout value
///
/// Reads from stdin and validates the input is a positive integer.
/// Returns 120 as the default if the user provides empty input.
///
/// # Errors
///
/// Returns `CliError::Io` if reading from stdin fails.
/// Returns `CliError::InvalidInput` if the input cannot be parsed as a positive integer.
pub fn prompt_test_timeout() -> Result<u32, CliError> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    write!(handle, "Enter test timeout in seconds (default: 120): ")?;
    handle.flush()?;

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    let trimmed = line.trim();

    // Empty input defaults to 120
    if trimmed.is_empty() {
        return Ok(120);
    }

    // Parse the input as u32
    let timeout = trimmed.parse::<u32>().map_err(|_| {
        CliError::InvalidInput(format!("'{}' is not a valid positive integer", trimmed))
    })?;

    // Validate it's positive (non-zero)
    if timeout == 0 {
        return Err(CliError::InvalidInput(
            "Timeout must be a positive integer (greater than 0)".to_string(),
        ));
    }

    Ok(timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() -> Result<(), Box<dyn std::error::Error>> {
        // Test parsing the apply command
        let cli = Cli::parse_from(["rust-bucket", "apply"]);
        match cli.command {
            Commands::Apply { force } => assert!(!force),
            other => return Err(format!("expected Apply, got {other:?}").into()),
        }

        // Test parsing the apply command with --force
        let cli = Cli::parse_from(["rust-bucket", "apply", "--force"]);
        match cli.command {
            Commands::Apply { force } => assert!(force),
            other => return Err(format!("expected Apply, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn test_show_migration_parsing() -> Result<(), Box<dyn std::error::Error>> {
        // Without flags both bounds are None.
        let cli = Cli::parse_from(["rust-bucket", "show-migration"]);
        match cli.command {
            Commands::ShowMigration { from, to } => {
                assert_eq!(from, None);
                assert_eq!(to, None);
            }
            other => return Err(format!("expected ShowMigration, got {other:?}").into()),
        }

        // With both --from and --to.
        let cli = Cli::parse_from([
            "rust-bucket",
            "show-migration",
            "--from",
            "0.5.0",
            "--to",
            "0.7.0",
        ]);
        match cli.command {
            Commands::ShowMigration { from, to } => {
                assert_eq!(from.as_deref(), Some("0.5.0"));
                assert_eq!(to.as_deref(), Some("0.7.0"));
            }
            other => return Err(format!("expected ShowMigration, got {other:?}").into()),
        }
        Ok(())
    }

    #[test]
    fn test_version_flag() {
        // Test that --version flag is recognized (clap will exit with code 0)
        let result = Cli::try_parse_from(["rust-bucket", "--version"]);
        // --version causes clap to print version and exit, which returns an error
        // of kind DisplayVersion
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }
}
