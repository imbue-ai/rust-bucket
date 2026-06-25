// Template generation and file creation

use crate::config::Config;
use crate::templates;
use liquid::ParserBuilder;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

/// Errors that can occur during template generation
#[derive(Debug, Error)]
pub enum GeneratorError {
    /// Error parsing or rendering a Liquid template
    #[error("Template error: {0}")]
    TemplateError(#[from] liquid::Error),

    /// IO error when reading or writing files
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// File conflicts detected when overwrite is disabled
    #[error("File conflicts detected (use overwrite=true to replace): {}", .0.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))]
    ConflictError(Vec<PathBuf>),

    /// Template directory does not exist or is not a directory
    #[error("Template directory not found or not a directory: {0}")]
    TemplateDirectoryError(String),

    /// Failed to determine relative path for template
    #[error("Failed to determine relative path for template: {0}")]
    PathError(String),
}

/// Render templates from a template directory to an output directory
///
/// # Arguments
/// * `template_dir` - Directory containing .liquid template files
/// * `output_dir` - Directory where rendered files will be written
/// * `config` - Configuration containing template variables (rust_bucket_version, test_timeout)
/// * `overwrite` - If false, fail if any target file exists. If true, replace existing files.
///
/// # Returns
/// A list of all generated file paths on success
///
/// # Errors
/// Returns `GeneratorError` if:
/// - Template directory doesn't exist or isn't readable
/// - Template parsing or rendering fails
/// - IO errors occur during file operations
/// - File conflicts are detected when overwrite=false
pub fn render(
    template_dir: &Path,
    output_dir: &Path,
    config: &Config,
    overwrite: bool,
) -> Result<Vec<PathBuf>, GeneratorError> {
    // Validate template directory exists
    if !template_dir.is_dir() {
        return Err(GeneratorError::TemplateDirectoryError(
            template_dir.display().to_string(),
        ));
    }

    // Create Liquid parser
    let parser = ParserBuilder::with_stdlib().build()?;

    // Prepare template variables from config
    let globals = liquid::object!({
        "rust_bucket_version": config.rust_bucket_version,
        "test_timeout": config.test_timeout,
        "project_name": config.project_name,
    });

    // Seed templates are written only via seed_files(); the managed render path
    // must never emit them.
    let seed_template_paths: Vec<&str> = templates::seed_files()
        .into_iter()
        .map(|(template, _)| template)
        .collect();

    // Track all files that will be generated
    let mut target_files = Vec::new();

    // First pass: collect all target files and check for conflicts
    for entry in WalkDir::new(template_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let template_path = entry.path();

        // Skip files that aren't .liquid templates
        if template_path.extension().is_none_or(|ext| ext != "liquid") {
            continue;
        }

        // Calculate relative path from template_dir
        let relative_path = template_path
            .strip_prefix(template_dir)
            .map_err(|e| GeneratorError::PathError(e.to_string()))?;

        // Skip seed templates; they are handled by seed_files().
        if seed_template_paths
            .iter()
            .any(|seed| Path::new(seed) == relative_path)
        {
            continue;
        }

        // Remove .liquid extension for output file
        let output_relative_path = relative_path.with_extension("");
        let output_path = output_dir.join(&output_relative_path);

        target_files.push(output_path);
    }

    // Check for conflicts if overwrite is disabled
    if !overwrite {
        let conflicts: Vec<PathBuf> = target_files
            .iter()
            .filter(|path| path.exists())
            .cloned()
            .collect();

        if !conflicts.is_empty() {
            return Err(GeneratorError::ConflictError(conflicts));
        }
    }

    // Second pass: render and write all templates
    let mut generated_files = Vec::new();

    for entry in WalkDir::new(template_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let template_path = entry.path();

        // Skip files that aren't .liquid templates
        if template_path.extension().is_none_or(|ext| ext != "liquid") {
            continue;
        }

        // Calculate relative path from template_dir
        let relative_path = template_path
            .strip_prefix(template_dir)
            .map_err(|e| GeneratorError::PathError(e.to_string()))?;

        // Skip seed templates; they are handled by seed_files().
        if seed_template_paths
            .iter()
            .any(|seed| Path::new(seed) == relative_path)
        {
            continue;
        }

        // Remove .liquid extension for output file
        let output_relative_path = relative_path.with_extension("");
        let output_path = output_dir.join(&output_relative_path);

        // Read template content
        let template_content = fs::read_to_string(template_path)?;

        // Parse and render template
        let template = parser.parse(&template_content)?;
        let rendered = template.render(&globals)?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write rendered content to output file
        fs::write(&output_path, rendered)?;

        generated_files.push(output_path);
    }

    Ok(generated_files)
}

/// Ensure .gitignore contains all required lines, appending any that are missing.
///
/// If no .gitignore exists, one is created with just the required lines.
/// Existing content is preserved; only missing lines are appended.
pub fn ensure_gitignore(target_dir: &Path) -> Result<Vec<String>, GeneratorError> {
    let gitignore_path = target_dir.join(".gitignore");
    let required = templates::required_gitignore_lines();

    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    let existing_lines: Vec<&str> = existing.lines().collect();
    let missing: Vec<&str> = required
        .iter()
        .filter(|line| !existing_lines.iter().any(|el| el.trim() == **line))
        .copied()
        .collect();

    if missing.is_empty() {
        return Ok(Vec::new());
    }

    let mut append = String::new();
    if !existing.is_empty() && !existing.ends_with('\n') {
        append.push('\n');
    }
    if !existing.is_empty() {
        append.push_str("\n# beads_rust (managed by rust-bucket)\n");
    }
    for line in &missing {
        append.push_str(line);
        append.push('\n');
    }

    fs::write(&gitignore_path, format!("{existing}{append}"))?;

    Ok(missing.iter().map(|s| s.to_string()).collect())
}

/// Render and write each registered seed template into the target, but only if
/// the destination file is absent. Existing files are left byte-for-byte
/// untouched, so seeding is safe to repeat on every apply.
///
/// # Returns
/// The destination paths that were newly seeded.
///
/// # Errors
/// Returns `GeneratorError` if a seed template cannot be read, parsed, rendered,
/// or written.
pub fn seed_files(
    template_dir: &Path,
    target_dir: &Path,
    config: &Config,
) -> Result<Vec<PathBuf>, GeneratorError> {
    let parser = ParserBuilder::with_stdlib().build()?;
    let globals = liquid::object!({
        "rust_bucket_version": config.rust_bucket_version,
        "test_timeout": config.test_timeout,
        "project_name": config.project_name,
    });

    let mut seeded = Vec::new();

    for (template_rel, dest_rel) in templates::seed_files() {
        let dest_path = target_dir.join(dest_rel);
        if dest_path.exists() {
            continue;
        }

        let template_path = template_dir.join(template_rel);
        let template_content = fs::read_to_string(&template_path)?;
        let template = parser.parse(&template_content)?;
        let rendered = template.render(&globals)?;

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest_path, rendered)?;
        seeded.push(dest_path);
    }

    Ok(seeded)
}

/// Check if a target directory contains a rust-bucket.toml marker file
///
/// # Arguments
/// * `target_dir` - Directory to check for the rust-bucket.toml file
///
/// # Returns
/// `true` if rust-bucket.toml exists in the target directory, `false` otherwise
pub fn has_rust_bucket_toml(target_dir: &Path) -> bool {
    target_dir.join("rust-bucket.toml").exists()
}

/// Check for conflicts between managed files and existing files in a target directory
///
/// # Arguments
/// * `target_dir` - Directory to check for conflicting files
///
/// # Returns
/// A vector of paths to files that would conflict with managed files.
/// Returns an empty vector if no conflicts are found.
pub fn check_conflicts(target_dir: &Path) -> Vec<PathBuf> {
    templates::managed_files()
        .iter()
        .map(|file| target_dir.join(file))
        .filter(|path| path.exists())
        .collect()
}

/// Create the CLAUDE.md symlink pointing to AGENTS.md
///
/// This creates a symbolic link at CLAUDE.md that points to AGENTS.md,
/// allowing Claude Code to find the agent instructions via its standard
/// CLAUDE.md lookup while keeping the canonical content in AGENTS.md.
///
/// # Arguments
/// * `target_dir` - Directory where the symlink should be created
///
/// # Errors
/// Returns `GeneratorError::IoError` if the symlink cannot be created
#[cfg(unix)]
pub fn create_claude_symlink(target_dir: &Path) -> Result<PathBuf, GeneratorError> {
    let claude_md = target_dir.join("CLAUDE.md");

    // Remove existing file or symlink if present
    if claude_md.exists() || claude_md.is_symlink() {
        fs::remove_file(&claude_md)?;
    }

    // Create symlink: CLAUDE.md -> AGENTS.md
    symlink("AGENTS.md", &claude_md)?;

    Ok(claude_md)
}

/// Create the CLAUDE.md symlink pointing to AGENTS.md (Windows version)
///
/// On Windows, we create a regular file copy instead of a symlink
/// since symlinks require elevated privileges.
#[cfg(windows)]
pub fn create_claude_symlink(target_dir: &Path) -> Result<PathBuf, GeneratorError> {
    let claude_md = target_dir.join("CLAUDE.md");
    let agents_md = target_dir.join("AGENTS.md");

    // Copy AGENTS.md to CLAUDE.md
    fs::copy(&agents_md, &claude_md)?;

    Ok(claude_md)
}

/// Mirror the canonical `.agents/skills/` tree into `.claude/skills/` via a single
/// directory symlink.
///
/// The canonical skill content lives under the vendor-neutral `.agents/skills/`
/// tree. Claude Code discovers skills under `.claude/skills/`, so the whole
/// directory is symlinked there — Agent Skills are the primary form, and the
/// Claude location is a thin pointer at them. A single `.claude/skills` symlink
/// means new skills appear automatically with no re-linking.
///
/// Returns the created `.claude/skills` symlink path, or `None` when no
/// `.agents/skills/` tree is present.
///
/// # Arguments
/// * `target_dir` - Repository root in which both trees live
///
/// # Errors
/// Returns `GeneratorError::IoError` if the symlink cannot be created.
#[cfg(unix)]
pub fn create_skill_symlinks(target_dir: &Path) -> Result<Option<PathBuf>, GeneratorError> {
    let agents_skills = target_dir.join(".agents/skills");
    if !agents_skills.is_dir() {
        return Ok(None);
    }

    fs::create_dir_all(target_dir.join(".claude"))?;
    let link = target_dir.join(".claude/skills");

    // On re-apply, remove the previous .claude/skills symlink before recreating it.
    if link.is_symlink() {
        fs::remove_file(&link)?;
    }

    // Relative target: from .claude/skills, one hop up reaches the repo root.
    symlink("../.agents/skills", &link)?;

    Ok(Some(link))
}

/// Mirror the canonical `.agents/skills/` tree into `.claude/skills/` (Windows version).
///
/// On Windows we copy the tree instead of symlinking it, since symlinks require
/// elevated privileges.
#[cfg(windows)]
pub fn create_skill_symlinks(target_dir: &Path) -> Result<Option<PathBuf>, GeneratorError> {
    let agents_skills = target_dir.join(".agents/skills");
    if !agents_skills.is_dir() {
        return Ok(None);
    }

    let claude_skills = target_dir.join(".claude/skills");
    if claude_skills.exists() {
        fs::remove_dir_all(&claude_skills)?;
    }
    copy_dir_all(&agents_skills, &claude_skills)?;

    Ok(Some(claude_skills))
}

/// Recursively copy a directory tree (Windows fallback for skill mirroring).
#[cfg(windows)]
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config() -> Config {
        Config {
            rust_bucket_version: "0.1.0".to_string(),
            test_timeout: 120,
            project_name: "test-project".to_string(),
        }
    }

    #[test]
    fn test_render_simple_template() -> Result<(), Box<dyn std::error::Error>> {
        let temp_template_dir = TempDir::new()?;
        let temp_output_dir = TempDir::new()?;

        // Create a simple template
        let template_path = temp_template_dir.path().join("test.txt.liquid");
        fs::write(
            &template_path,
            "Version: {{ rust_bucket_version }}\nTimeout: {{ test_timeout }}s",
        )?;

        let config = create_test_config();
        let generated_files = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false,
        )?;

        assert_eq!(generated_files.len(), 1);

        let output_path = temp_output_dir.path().join("test.txt");
        assert!(output_path.exists());

        let content = fs::read_to_string(&output_path)?;
        assert_eq!(content, "Version: 0.1.0\nTimeout: 120s");
        Ok(())
    }

    #[test]
    fn test_render_nested_template() -> Result<(), Box<dyn std::error::Error>> {
        let temp_template_dir = TempDir::new()?;
        let temp_output_dir = TempDir::new()?;

        // Create a nested directory structure
        let subdir = temp_template_dir.path().join("subdir");
        fs::create_dir(&subdir)?;

        let template_path = subdir.join("nested.txt.liquid");
        fs::write(&template_path, "Nested: {{ rust_bucket_version }}")?;

        let config = create_test_config();
        render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false,
        )?;

        let output_path = temp_output_dir.path().join("subdir/nested.txt");
        assert!(output_path.exists());

        let content = fs::read_to_string(&output_path)?;
        assert_eq!(content, "Nested: 0.1.0");
        Ok(())
    }

    #[test]
    fn test_conflict_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_template_dir = TempDir::new()?;
        let temp_output_dir = TempDir::new()?;

        // Create a template
        let template_path = temp_template_dir.path().join("test.txt.liquid");
        fs::write(&template_path, "Content: {{ rust_bucket_version }}")?;

        // Create a conflicting file in output directory
        let output_path = temp_output_dir.path().join("test.txt");
        fs::write(&output_path, "existing content")?;

        let config = create_test_config();
        let result = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false, // overwrite disabled
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, GeneratorError::ConflictError(_)),
            "Expected ConflictError"
        );
        if let GeneratorError::ConflictError(conflicts) = err {
            assert_eq!(conflicts.len(), 1);
            assert!(conflicts[0].ends_with("test.txt"));
        }
        Ok(())
    }

    #[test]
    fn test_overwrite_existing_files() -> Result<(), Box<dyn std::error::Error>> {
        let temp_template_dir = TempDir::new()?;
        let temp_output_dir = TempDir::new()?;

        // Create a template
        let template_path = temp_template_dir.path().join("test.txt.liquid");
        fs::write(&template_path, "New: {{ rust_bucket_version }}")?;

        // Create a conflicting file in output directory
        let output_path = temp_output_dir.path().join("test.txt");
        fs::write(&output_path, "old content")?;

        let config = create_test_config();
        render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            true, // overwrite enabled
        )?;

        // Verify file was overwritten
        let content = fs::read_to_string(&output_path)?;
        assert_eq!(content, "New: 0.1.0");
        assert_ne!(content, "old content");
        Ok(())
    }

    #[test]
    fn test_nonexistent_template_directory() -> Result<(), Box<dyn std::error::Error>> {
        let temp_output_dir = TempDir::new()?;
        let nonexistent_dir = PathBuf::from("/nonexistent/template/dir");

        let config = create_test_config();
        let result = render(&nonexistent_dir, temp_output_dir.path(), &config, false);

        assert!(result.is_err());
        assert!(
            matches!(
                result.unwrap_err(),
                GeneratorError::TemplateDirectoryError(_)
            ),
            "Expected TemplateDirectoryError"
        );
        Ok(())
    }

    #[test]
    fn test_skip_non_liquid_files() -> Result<(), Box<dyn std::error::Error>> {
        let temp_template_dir = TempDir::new()?;
        let temp_output_dir = TempDir::new()?;

        // Create a .liquid template
        let liquid_path = temp_template_dir.path().join("template.txt.liquid");
        fs::write(&liquid_path, "Version: {{ rust_bucket_version }}")?;

        // Create a non-.liquid file that should be skipped
        let non_liquid_path = temp_template_dir.path().join("regular.txt");
        fs::write(&non_liquid_path, "This should be skipped")?;

        let config = create_test_config();
        let generated_files = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false,
        )?;

        // Should only generate from .liquid files
        assert_eq!(generated_files.len(), 1);
        assert!(generated_files[0].ends_with("template.txt"));

        // The non-.liquid file should not be copied
        let skipped_path = temp_output_dir.path().join("regular.txt");
        assert!(!skipped_path.exists());
        Ok(())
    }

    #[test]
    fn test_template_syntax_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_template_dir = TempDir::new()?;
        let temp_output_dir = TempDir::new()?;

        // Create a template with invalid Liquid syntax
        let template_path = temp_template_dir.path().join("bad.txt.liquid");
        fs::write(&template_path, "Bad syntax: {{ unclosed_tag")?;

        let config = create_test_config();
        let result = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false,
        );

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), GeneratorError::TemplateError(_)),
            "Expected TemplateError"
        );
        Ok(())
    }

    #[test]
    fn test_has_rust_bucket_toml_exists() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let toml_path = temp_dir.path().join("rust-bucket.toml");

        // Initially should not exist
        assert!(!has_rust_bucket_toml(temp_dir.path()));

        // Create the file
        fs::write(&toml_path, "test_content")?;

        // Now it should exist
        assert!(has_rust_bucket_toml(temp_dir.path()));
        Ok(())
    }

    #[test]
    fn test_has_rust_bucket_toml_not_exists() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        assert!(!has_rust_bucket_toml(temp_dir.path()));
        Ok(())
    }

    #[test]
    fn test_check_conflicts_no_conflicts() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let conflicts = check_conflicts(temp_dir.path());
        assert!(conflicts.is_empty());
        Ok(())
    }

    #[test]
    fn test_check_conflicts_with_conflicts() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        // Create some managed files that would conflict
        fs::write(temp_dir.path().join("AGENTS.md"), "existing content")?;
        fs::write(
            temp_dir.path().join("RUST_STYLE_GUIDE.md"),
            "existing content",
        )?;

        // Create .devcontainer directory and file
        let devcontainer_dir = temp_dir.path().join(".devcontainer");
        fs::create_dir(&devcontainer_dir)?;
        fs::write(devcontainer_dir.join("Dockerfile"), "existing content")?;

        let conflicts = check_conflicts(temp_dir.path());

        // Should detect the conflicts
        assert!(!conflicts.is_empty());
        assert_eq!(conflicts.len(), 3);

        // Verify the conflicting files are in the list
        let conflict_names: Vec<String> = conflicts
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();

        assert!(conflict_names.contains(&"AGENTS.md".to_string()));
        assert!(conflict_names.contains(&"RUST_STYLE_GUIDE.md".to_string()));
        assert!(conflict_names.contains(&"Dockerfile".to_string()));
        Ok(())
    }

    #[test]
    fn test_check_conflicts_partial_conflicts() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        // Create only one managed file
        fs::create_dir_all(temp_dir.path().join(".claude/agents"))?;
        fs::write(
            temp_dir.path().join(".claude/agents/coordinator.md"),
            "existing content",
        )?;

        let conflicts = check_conflicts(temp_dir.path());

        // Should detect exactly one conflict
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].ends_with(".claude/agents/coordinator.md"));
        Ok(())
    }

    #[test]
    fn test_ensure_gitignore_creates_file_when_missing() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let added = ensure_gitignore(temp_dir.path())?;
        assert_eq!(added.len(), 4);
        let content = fs::read_to_string(temp_dir.path().join(".gitignore"))?;
        assert!(content.contains(".beads/.br_history/"));
        assert!(content.contains(".beads/beads.db-wal"));
        Ok(())
    }

    #[test]
    fn test_ensure_gitignore_appends_missing_lines() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join(".gitignore"), "target/\n")?;
        let added = ensure_gitignore(temp_dir.path())?;
        assert_eq!(added.len(), 4);
        let content = fs::read_to_string(temp_dir.path().join(".gitignore"))?;
        assert!(content.starts_with("target/\n"));
        assert!(content.contains("# beads_rust (managed by rust-bucket)"));
        assert!(content.contains(".beads/beads.db"));
        Ok(())
    }

    #[test]
    fn test_ensure_gitignore_skips_existing_lines() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        fs::write(
            temp_dir.path().join(".gitignore"),
            "target/\n.beads/.br_history/\n.beads/beads.db\n.beads/beads.db-wal\n.beads/last-touched\n",
        )?;
        let added = ensure_gitignore(temp_dir.path())?;
        assert!(added.is_empty());
        Ok(())
    }

    #[test]
    fn test_seed_files_writes_when_absent() -> Result<(), Box<dyn std::error::Error>> {
        let temp_template_dir = TempDir::new()?;
        let temp_target_dir = TempDir::new()?;

        let template_path = temp_template_dir.path().join("ratchets.toml.liquid");
        fs::write(&template_path, "enabled_ratchets = []\n")?;
        let style_template = temp_template_dir.path().join("STYLE_GUIDE.md.liquid");
        fs::write(&style_template, "# Style Guide\n")?;

        let config = create_test_config();
        let seeded = seed_files(temp_template_dir.path(), temp_target_dir.path(), &config)?;

        let dest = temp_target_dir.path().join("ratchets.toml");
        assert!(dest.exists());
        assert!(seeded.contains(&dest));
        assert_eq!(fs::read_to_string(&dest)?, "enabled_ratchets = []\n");
        Ok(())
    }

    #[test]
    fn test_seed_files_leaves_existing_unchanged() -> Result<(), Box<dyn std::error::Error>> {
        let temp_template_dir = TempDir::new()?;
        let temp_target_dir = TempDir::new()?;

        let template_path = temp_template_dir.path().join("ratchets.toml.liquid");
        fs::write(&template_path, "enabled_ratchets = []\n")?;
        let style_template = temp_template_dir.path().join("STYLE_GUIDE.md.liquid");
        fs::write(&style_template, "# Style Guide\n")?;

        let dest = temp_target_dir.path().join("ratchets.toml");
        let custom = "enabled_ratchets = [\"no-unwrap\"]\n# customized\n";
        fs::write(&dest, custom)?;
        let style_dest = temp_target_dir.path().join("STYLE_GUIDE.md");
        fs::write(&style_dest, "# existing style\n")?;

        let config = create_test_config();
        let seeded = seed_files(temp_template_dir.path(), temp_target_dir.path(), &config)?;

        assert!(seeded.is_empty());
        assert_eq!(fs::read_to_string(&dest)?, custom);
        Ok(())
    }

    #[test]
    fn test_seed_files_writes_style_guide_when_absent() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp_dir, temp_path) = templates::extract_to_temp()?;
        let temp_target_dir = TempDir::new()?;

        let config = create_test_config();
        let seeded = seed_files(&temp_path, temp_target_dir.path(), &config)?;

        let dest = temp_target_dir.path().join("STYLE_GUIDE.md");
        assert!(dest.exists());
        assert!(seeded.contains(&dest));

        let content = fs::read_to_string(&dest)?;
        assert!(content.starts_with("# Style Guide\n"));
        assert!(content.contains("RUST_STYLE_GUIDE.md"));
        assert!(!content.contains("Generated by rust-bucket"));
        Ok(())
    }

    #[test]
    fn test_seed_files_leaves_existing_style_guide_unchanged()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_temp_dir, temp_path) = templates::extract_to_temp()?;
        let temp_target_dir = TempDir::new()?;

        let dest = temp_target_dir.path().join("STYLE_GUIDE.md");
        let custom = "<!-- Generated by rust-bucket v0.7.0. DO NOT EDIT BY HAND. -->\n# Custom\n";
        fs::write(&dest, custom)?;

        let config = create_test_config();
        let seeded = seed_files(&temp_path, temp_target_dir.path(), &config)?;

        assert!(!seeded.contains(&dest));
        assert_eq!(fs::read_to_string(&dest)?, custom);
        Ok(())
    }

    #[test]
    fn test_render_skips_seed_templates() -> Result<(), Box<dyn std::error::Error>> {
        let temp_template_dir = TempDir::new()?;
        let temp_output_dir = TempDir::new()?;

        let seed_template = temp_template_dir.path().join("ratchets.toml.liquid");
        fs::write(&seed_template, "enabled_ratchets = []\n")?;

        let managed_template = temp_template_dir.path().join("AGENTS.md.liquid");
        fs::write(&managed_template, "Version: {{ rust_bucket_version }}")?;

        let config = create_test_config();
        let generated = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false,
        )?;

        assert!(
            !temp_output_dir.path().join("ratchets.toml").exists(),
            "render must not emit seed templates"
        );
        assert!(generated.iter().any(|p| p.ends_with("AGENTS.md")));
        assert!(!generated.iter().any(|p| p.ends_with("ratchets.toml")));
        Ok(())
    }

    #[test]
    fn test_ensure_gitignore_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        fs::write(temp_dir.path().join(".gitignore"), "target/\n")?;
        ensure_gitignore(temp_dir.path())?;
        let first = fs::read_to_string(temp_dir.path().join(".gitignore"))?;
        let added = ensure_gitignore(temp_dir.path())?;
        assert!(added.is_empty());
        let second = fs::read_to_string(temp_dir.path().join(".gitignore"))?;
        assert_eq!(first, second);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_create_skill_symlinks_links_agents_into_claude()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let skill_dir = temp_dir.path().join(".agents/skills/release-to-crates");
        fs::create_dir_all(&skill_dir)?;
        fs::write(skill_dir.join("SKILL.md"), "canonical content\n")?;

        let created = create_skill_symlinks(temp_dir.path())?;

        let link = temp_dir.path().join(".claude/skills");
        assert_eq!(created.as_ref(), Some(&link));
        assert!(link.is_symlink(), ".claude/skills should be a symlink");
        assert_eq!(fs::read_link(&link)?, Path::new("../.agents/skills"));
        // The symlink must resolve to the canonical content.
        assert_eq!(
            fs::read_to_string(link.join("release-to-crates/SKILL.md"))?,
            "canonical content\n"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_create_skill_symlinks_noop_without_agents_dir() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = TempDir::new()?;
        let created = create_skill_symlinks(temp_dir.path())?;
        assert!(created.is_none());
        assert!(!temp_dir.path().join(".claude/skills").exists());
        Ok(())
    }
}
