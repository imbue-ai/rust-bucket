#![forbid(unsafe_code)]

use clap::Parser;
use rust_bucket::{apply, cli, generator, verify};
use std::process;

fn main() {
    let result = run();

    match result {
        Ok(exit_code) => process::exit(exit_code),
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

/// Main application logic, returning an exit code
fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Commands::Apply { force } => {
            let target_dir = std::env::current_dir()?;

            let result = if generator::has_rust_bucket_toml(&target_dir) {
                apply::apply_update(&target_dir)
            } else {
                apply::apply_init(&target_dir, force)
            }?;

            // Print results
            print_results(&result);

            // Determine exit code based on verification
            if result.verification.is_success() {
                Ok(0)
            } else {
                Ok(1)
            }
        }
    }
}

/// Pretty print the apply results
fn print_results(result: &apply::ApplyResult) {
    println!("\nGenerated {} file(s):", result.files_generated.len());
    for file in &result.files_generated {
        println!("  - {}", file.display());
    }

    println!("\nVerification Results:");
    print_step_result("Format check", &result.verification.format);
    print_step_result("Clippy", &result.verification.clippy);
    print_step_result("Tests", &result.verification.test);
    print_step_result("Ratchets", &result.verification.ratchets);

    if result.verification.is_success() {
        println!("\n✓ All checks passed!");
    } else {
        println!("\n✗ Some checks failed. Please review the output above.");
    }
}

/// Print a single verification step result
fn print_step_result(name: &str, result: &verify::StepResult) {
    match result {
        verify::StepResult::Pass => {
            println!("  ✓ {}: PASS", name);
        }
        verify::StepResult::Skip(reason) => {
            println!("  ⊘ {}: SKIPPED ({})", name, reason);
        }
        verify::StepResult::Fail(message) => {
            println!("  ✗ {}: FAIL", name);
            // Print failure details indented
            for line in message.lines() {
                println!("    {}", line);
            }
        }
    }
}
