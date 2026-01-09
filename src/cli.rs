// CLI argument parsing and command dispatch

use clap::{Parser, Subcommand};
use std::io::{self, BufRead, Write};
use thiserror::Error;

/// Rust-first project bootstrapper for AI-first engineering
#[derive(Parser)]
#[command(name = "rust-bucket")]
#[command(about = "Rust-first project bootstrapper for AI-first engineering")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands
#[derive(Subcommand)]
pub enum Commands {
    /// Apply rust-bucket to the current directory
    Apply {
        /// Force overwrite of existing managed files
        #[arg(long)]
        force: bool,
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
    fn test_cli_parsing() {
        // Test parsing the apply command
        let cli = Cli::parse_from(["rust-bucket", "apply"]);
        match cli.command {
            Commands::Apply { force } => assert!(!force),
        }

        // Test parsing the apply command with --force
        let cli = Cli::parse_from(["rust-bucket", "apply", "--force"]);
        match cli.command {
            Commands::Apply { force } => assert!(force),
        }
    }
}
