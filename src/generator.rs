// Template generation and file creation

use crate::config::Config;
use liquid::ParserBuilder;
use std::fs;
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
    });

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config() -> Config {
        Config {
            rust_bucket_version: "0.1.0".to_string(),
            test_timeout: 120,
        }
    }

    #[test]
    fn test_render_simple_template() {
        let temp_template_dir = TempDir::new().unwrap();
        let temp_output_dir = TempDir::new().unwrap();

        // Create a simple template
        let template_path = temp_template_dir.path().join("test.txt.liquid");
        fs::write(
            &template_path,
            "Version: {{ rust_bucket_version }}\nTimeout: {{ test_timeout }}s",
        )
        .unwrap();

        let config = create_test_config();
        let result = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false,
        );

        assert!(result.is_ok());
        let generated_files = result.unwrap();
        assert_eq!(generated_files.len(), 1);

        let output_path = temp_output_dir.path().join("test.txt");
        assert!(output_path.exists());

        let content = fs::read_to_string(&output_path).unwrap();
        assert_eq!(content, "Version: 0.1.0\nTimeout: 120s");
    }

    #[test]
    fn test_render_nested_template() {
        let temp_template_dir = TempDir::new().unwrap();
        let temp_output_dir = TempDir::new().unwrap();

        // Create a nested directory structure
        let subdir = temp_template_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let template_path = subdir.join("nested.txt.liquid");
        fs::write(&template_path, "Nested: {{ rust_bucket_version }}").unwrap();

        let config = create_test_config();
        let result = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false,
        );

        assert!(result.is_ok());

        let output_path = temp_output_dir.path().join("subdir/nested.txt");
        assert!(output_path.exists());

        let content = fs::read_to_string(&output_path).unwrap();
        assert_eq!(content, "Nested: 0.1.0");
    }

    #[test]
    fn test_conflict_detection() {
        let temp_template_dir = TempDir::new().unwrap();
        let temp_output_dir = TempDir::new().unwrap();

        // Create a template
        let template_path = temp_template_dir.path().join("test.txt.liquid");
        fs::write(&template_path, "Content: {{ rust_bucket_version }}").unwrap();

        // Create a conflicting file in output directory
        let output_path = temp_output_dir.path().join("test.txt");
        fs::write(&output_path, "existing content").unwrap();

        let config = create_test_config();
        let result = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false, // overwrite disabled
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            GeneratorError::ConflictError(conflicts) => {
                assert_eq!(conflicts.len(), 1);
                assert!(conflicts[0].ends_with("test.txt"));
            }
            _ => panic!("Expected ConflictError"),
        }
    }

    #[test]
    fn test_overwrite_existing_files() {
        let temp_template_dir = TempDir::new().unwrap();
        let temp_output_dir = TempDir::new().unwrap();

        // Create a template
        let template_path = temp_template_dir.path().join("test.txt.liquid");
        fs::write(&template_path, "New: {{ rust_bucket_version }}").unwrap();

        // Create a conflicting file in output directory
        let output_path = temp_output_dir.path().join("test.txt");
        fs::write(&output_path, "old content").unwrap();

        let config = create_test_config();
        let result = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            true, // overwrite enabled
        );

        assert!(result.is_ok());

        // Verify file was overwritten
        let content = fs::read_to_string(&output_path).unwrap();
        assert_eq!(content, "New: 0.1.0");
        assert_ne!(content, "old content");
    }

    #[test]
    fn test_nonexistent_template_directory() {
        let temp_output_dir = TempDir::new().unwrap();
        let nonexistent_dir = PathBuf::from("/nonexistent/template/dir");

        let config = create_test_config();
        let result = render(
            &nonexistent_dir,
            temp_output_dir.path(),
            &config,
            false,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            GeneratorError::TemplateDirectoryError(_) => {}
            _ => panic!("Expected TemplateDirectoryError"),
        }
    }

    #[test]
    fn test_skip_non_liquid_files() {
        let temp_template_dir = TempDir::new().unwrap();
        let temp_output_dir = TempDir::new().unwrap();

        // Create a .liquid template
        let liquid_path = temp_template_dir.path().join("template.txt.liquid");
        fs::write(&liquid_path, "Version: {{ rust_bucket_version }}").unwrap();

        // Create a non-.liquid file that should be skipped
        let non_liquid_path = temp_template_dir.path().join("regular.txt");
        fs::write(&non_liquid_path, "This should be skipped").unwrap();

        let config = create_test_config();
        let result = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false,
        );

        assert!(result.is_ok());
        let generated_files = result.unwrap();

        // Should only generate from .liquid files
        assert_eq!(generated_files.len(), 1);
        assert!(generated_files[0].ends_with("template.txt"));

        // The non-.liquid file should not be copied
        let skipped_path = temp_output_dir.path().join("regular.txt");
        assert!(!skipped_path.exists());
    }

    #[test]
    fn test_template_syntax_error() {
        let temp_template_dir = TempDir::new().unwrap();
        let temp_output_dir = TempDir::new().unwrap();

        // Create a template with invalid Liquid syntax
        let template_path = temp_template_dir.path().join("bad.txt.liquid");
        fs::write(&template_path, "Bad syntax: {{ unclosed_tag").unwrap();

        let config = create_test_config();
        let result = render(
            temp_template_dir.path(),
            temp_output_dir.path(),
            &config,
            false,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            GeneratorError::TemplateError(_) => {}
            _ => panic!("Expected TemplateError"),
        }
    }
}
