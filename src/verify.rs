// Verification and validation logic

use std::path::Path;
use std::process::Command;
use thiserror::Error;

/// Verification step types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyStep {
    Format,
    Clippy,
    Test,
}

/// Result of running a verification step
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepResult {
    Pass,
    Fail(String),
    Skip(String),
}

/// Report containing results of all verification steps
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub format: StepResult,
    pub clippy: StepResult,
    pub test: StepResult,
}

/// Errors that can occur during verification
#[derive(Error, Debug)]
pub enum VerifyError {
    #[error("Failed to execute cargo command: {0}")]
    CommandExecution(String),

    #[error("I/O error during verification: {0}")]
    Io(#[from] std::io::Error),
}

impl VerifyReport {
    /// Check if all verification steps passed or were skipped
    pub fn is_success(&self) -> bool {
        matches!(self.format, StepResult::Pass | StepResult::Skip(_))
            && matches!(self.clippy, StepResult::Pass | StepResult::Skip(_))
            && matches!(self.test, StepResult::Pass | StepResult::Skip(_))
    }
}

/// Run all verification steps on the target directory
pub fn run_all(target_dir: &Path) -> Result<VerifyReport, VerifyError> {
    let format = run_format_check(target_dir)?;
    let clippy = run_clippy(target_dir)?;
    let test = run_tests(target_dir)?;

    Ok(VerifyReport {
        format,
        clippy,
        test,
    })
}

/// Run cargo fmt --check
fn run_format_check(target_dir: &Path) -> Result<StepResult, VerifyError> {
    let output = Command::new("cargo")
        .arg("fmt")
        .arg("--check")
        .current_dir(target_dir)
        .output()?;

    if output.status.success() {
        Ok(StepResult::Pass)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let message = format!("{}\n{}", stdout, stderr).trim().to_string();
        Ok(StepResult::Fail(message))
    }
}

/// Run cargo clippy --all-targets --all-features
fn run_clippy(target_dir: &Path) -> Result<StepResult, VerifyError> {
    let output = Command::new("cargo")
        .arg("clippy")
        .arg("--all-targets")
        .arg("--all-features")
        .current_dir(target_dir)
        .output()?;

    if output.status.success() {
        Ok(StepResult::Pass)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let message = format!("{}\n{}", stdout, stderr).trim().to_string();
        Ok(StepResult::Fail(message))
    }
}

/// Run cargo nextest run
fn run_tests(target_dir: &Path) -> Result<StepResult, VerifyError> {
    // First check if cargo-nextest is available
    let nextest_check = Command::new("cargo")
        .arg("nextest")
        .arg("--version")
        .output();

    match nextest_check {
        Ok(output) if output.status.success() => {
            // cargo-nextest is available, run tests
            let test_output = Command::new("cargo")
                .arg("nextest")
                .arg("run")
                .current_dir(target_dir)
                .output()?;

            if test_output.status.success() {
                Ok(StepResult::Pass)
            } else {
                let stderr = String::from_utf8_lossy(&test_output.stderr);
                let stdout = String::from_utf8_lossy(&test_output.stdout);
                let message = format!("{}\n{}", stdout, stderr).trim().to_string();
                Ok(StepResult::Fail(message))
            }
        }
        Ok(_) | Err(_) => {
            // cargo-nextest not installed
            Ok(StepResult::Skip("cargo-nextest not installed".to_string()))
        }
    }
}
