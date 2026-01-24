// Integration tests for rust-bucket full workflow

use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper function to create a minimal Rust crate with git repo in a temp directory
///
/// Creates:
/// - Cargo.toml with basic package metadata
/// - .git/ directory (to simulate git init)
/// - src/ directory with lib.rs
fn create_test_rust_crate(path: &Path) {
    // Create Cargo.toml
    fs::write(
        path.join("Cargo.toml"),
        r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2024"
"#,
    )
    .unwrap();

    // Create .git directory to simulate git init
    fs::create_dir(path.join(".git")).unwrap();

    // Create src directory with minimal lib.rs
    let src_dir = path.join("src");
    fs::create_dir(&src_dir).unwrap();
    fs::write(src_dir.join("lib.rs"), "// test lib\n").unwrap();
}

/// Helper function to create a mock CLI input for test_timeout prompt
///
/// Since tests don't have interactive input, we'll need to work around the CLI prompt.
/// For these integration tests, we'll call apply functions with pre-set configs.
fn create_test_config() -> rust_bucket::config::Config {
    rust_bucket::config::Config {
        rust_bucket_version: env!("CARGO_PKG_VERSION").to_string(),
        test_timeout: 120,
        project_name: "test-project".to_string(),
    }
}

#[test]
fn test_init_on_fresh_repo() {
    // Create temp dir with Cargo.toml and .git/
    let temp_dir = TempDir::new().unwrap();
    create_test_rust_crate(temp_dir.path());

    // Note: We cannot directly test apply_init because it requires interactive prompt_test_timeout
    // Instead, we'll test the underlying functions that apply_init uses

    // Verify preconditions
    assert!(temp_dir.path().join("Cargo.toml").exists());
    assert!(temp_dir.path().join(".git").exists());

    // Check no conflicts exist initially
    let conflicts = rust_bucket::generator::check_conflicts(temp_dir.path());
    assert!(conflicts.is_empty(), "Fresh repo should have no conflicts");

    // Create and save config
    let config = create_test_config();
    let config_path = temp_dir.path().join("rust-bucket.toml");
    config.save(&config_path).unwrap();

    // Extract templates and render
    let (_temp_template_dir, template_path) = rust_bucket::templates::extract_to_temp().unwrap();
    let files_generated =
        rust_bucket::generator::render(&template_path, temp_dir.path(), &config, false).unwrap();

    // Assert all managed files were created
    let managed_files = rust_bucket::templates::managed_files();
    assert_eq!(
        files_generated.len(),
        managed_files.len(),
        "Should generate all managed files"
    );

    // Verify each managed file exists
    for file in managed_files {
        let file_path = temp_dir.path().join(file);
        assert!(
            file_path.exists(),
            "Managed file should exist: {}",
            file_path.display()
        );
    }

    // Assert rust-bucket.toml was created
    assert!(config_path.exists(), "rust-bucket.toml should exist");

    // Verify config can be loaded back
    let loaded_config = rust_bucket::config::Config::load(&config_path).unwrap();
    assert_eq!(loaded_config.test_timeout, 120);
}

#[test]
fn test_init_fails_on_conflict() {
    // Create temp dir with existing AGENTS.md
    let temp_dir = TempDir::new().unwrap();
    create_test_rust_crate(temp_dir.path());

    // Create a conflicting file
    fs::write(temp_dir.path().join("AGENTS.md"), "existing content").unwrap();

    // Check conflicts are detected
    let conflicts = rust_bucket::generator::check_conflicts(temp_dir.path());
    assert!(!conflicts.is_empty(), "Should detect conflict");
    assert_eq!(conflicts.len(), 1, "Should detect exactly one conflict");
    assert!(
        conflicts[0].ends_with("AGENTS.md"),
        "Conflict should be AGENTS.md"
    );

    // Try to render without force (overwrite=false) - should fail
    let config = create_test_config();
    let (_temp_template_dir, template_path) = rust_bucket::templates::extract_to_temp().unwrap();

    let result = rust_bucket::generator::render(&template_path, temp_dir.path(), &config, false);

    // Assert error with conflict list
    assert!(result.is_err(), "Render should fail on conflict");
    match result.unwrap_err() {
        rust_bucket::generator::GeneratorError::ConflictError(conflict_list) => {
            assert_eq!(conflict_list.len(), 1);
            assert!(conflict_list[0].ends_with("AGENTS.md"));
        }
        e => panic!("Expected ConflictError, got: {:?}", e),
    }
}

#[test]
fn test_init_force_overwrites() {
    // Create temp dir with existing AGENTS.md
    let temp_dir = TempDir::new().unwrap();
    create_test_rust_crate(temp_dir.path());

    // Create a conflicting file with known content
    let agents_path = temp_dir.path().join("AGENTS.md");
    fs::write(&agents_path, "OLD CONTENT SHOULD BE REPLACED").unwrap();

    // Verify the old content exists
    let old_content = fs::read_to_string(&agents_path).unwrap();
    assert_eq!(old_content, "OLD CONTENT SHOULD BE REPLACED");

    // Run render with force=true (overwrite=true)
    let config = create_test_config();
    let (_temp_template_dir, template_path) = rust_bucket::templates::extract_to_temp().unwrap();
    let result = rust_bucket::generator::render(&template_path, temp_dir.path(), &config, true);

    // Assert generation succeeded
    assert!(result.is_ok(), "Render with force should succeed");

    // Assert AGENTS.md was overwritten
    assert!(agents_path.exists(), "AGENTS.md should still exist");
    let new_content = fs::read_to_string(&agents_path).unwrap();

    // Verify content was replaced (should contain version stamp and not old content)
    assert_ne!(
        new_content, old_content,
        "Content should have been overwritten"
    );
    assert!(
        new_content.contains("Generated by rust-bucket"),
        "New content should have version stamp"
    );
    assert!(
        new_content.contains("Guide for Agents"),
        "New content should have expected AGENTS.md content"
    );
    assert!(
        !new_content.contains("OLD CONTENT"),
        "Old content should be gone"
    );
}

#[test]
fn test_update_preserves_config() {
    // Create temp dir with rust-bucket.toml (custom timeout)
    let temp_dir = TempDir::new().unwrap();
    create_test_rust_crate(temp_dir.path());

    // Create initial config with custom timeout
    let mut config = create_test_config();
    config.test_timeout = 300; // Custom timeout
    let config_path = temp_dir.path().join("rust-bucket.toml");
    config.save(&config_path).unwrap();

    // Extract templates and render (simulating first init)
    let (_temp_template_dir1, template_path1) = rust_bucket::templates::extract_to_temp().unwrap();
    rust_bucket::generator::render(&template_path1, temp_dir.path(), &config, false).unwrap();

    // Verify nextest.toml contains custom timeout
    let nextest_path = temp_dir.path().join(".config/nextest.toml");
    let nextest_content = fs::read_to_string(&nextest_path).unwrap();
    assert!(
        nextest_content.contains("300s"),
        "Initial nextest.toml should have 300s timeout"
    );

    // Now simulate an update: load config, update version, re-render with overwrite=true
    let mut loaded_config = rust_bucket::config::Config::load(&config_path).unwrap();
    assert_eq!(
        loaded_config.test_timeout, 300,
        "Loaded config should preserve custom timeout"
    );

    // Update the rust_bucket_version (simulating update flow)
    loaded_config.rust_bucket_version = "0.2.0".to_string();
    loaded_config.save(&config_path).unwrap();

    // Re-render templates with overwrite=true
    let (_temp_template_dir2, template_path2) = rust_bucket::templates::extract_to_temp().unwrap();
    rust_bucket::generator::render(&template_path2, temp_dir.path(), &loaded_config, true).unwrap();

    // Assert timeout preserved in regenerated files
    let updated_nextest_content = fs::read_to_string(&nextest_path).unwrap();
    assert!(
        updated_nextest_content.contains("300s"),
        "Updated nextest.toml should still have 300s timeout"
    );

    // Verify version was updated in config
    let final_config = rust_bucket::config::Config::load(&config_path).unwrap();
    assert_eq!(final_config.rust_bucket_version, "0.2.0");
    assert_eq!(
        final_config.test_timeout, 300,
        "Timeout should be preserved"
    );
}

#[test]
fn test_version_stamp_in_generated_files() {
    // Run apply_init (or just render)
    let temp_dir = TempDir::new().unwrap();
    create_test_rust_crate(temp_dir.path());

    let config = create_test_config();
    let (_temp_template_dir, template_path) = rust_bucket::templates::extract_to_temp().unwrap();
    rust_bucket::generator::render(&template_path, temp_dir.path(), &config, false).unwrap();

    // Check each generated file has version comment or stamp
    let files_to_check = vec![
        ("AGENTS.md", "<!-- Generated by rust-bucket", true),
        ("STYLE_GUIDE.md", "<!-- Generated by rust-bucket", true),
        ("TESTING.md", "<!-- Generated by rust-bucket", true),
        (
            ".claude/agents/coordinator.md",
            "<!-- Generated by rust-bucket",
            true,
        ),
        (".config/nextest.toml", "# Generated by rust-bucket", true),
        ("deny.toml", "# Generated by rust-bucket", true),
        ("rustfmt.toml", "# Generated by rust-bucket", true),
        (
            ".devcontainer/Dockerfile",
            "# Generated by rust-bucket",
            true,
        ),
        (
            ".devcontainer/devcontainer.json",
            "rust-bucket v",
            false, // JSON uses _generated field instead of comment
        ),
        (".beads/config.yaml", "# Generated by rust-bucket", true),
    ];

    for (file_path, expected_stamp_text, should_have_do_not_edit) in files_to_check {
        let full_path = temp_dir.path().join(file_path);
        assert!(full_path.exists(), "File should exist: {}", file_path);

        let content = fs::read_to_string(&full_path).unwrap();

        // Verify version stamp exists somewhere in the file
        assert!(
            content.contains(expected_stamp_text),
            "File {} should contain version stamp '{}', but content is:\n{}",
            file_path,
            expected_stamp_text,
            content.lines().take(5).collect::<Vec<_>>().join("\n")
        );

        // Verify it contains the version number
        assert!(
            content.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
            "File {} should contain version number v{}",
            file_path,
            env!("CARGO_PKG_VERSION")
        );

        // Verify it contains DO NOT EDIT warning (except for JSON files which use different format)
        if should_have_do_not_edit {
            assert!(
                content.contains("DO NOT EDIT"),
                "File {} should contain DO NOT EDIT warning",
                file_path
            );
        }
    }
}
