//! Plugin review via LLM providers.
//!
//! Reads plugin content (plugin.json, extends-plugin.json, README, skills),
//! builds a structured prompt, sends it to an LLM provider, and optionally
//! saves the resulting review report to disk.

use std::path::Path;

use crate::error::SoukError;
use crate::resolution::skill::enumerate_skills;
use crate::review::provider::LlmProvider;

/// The result of reviewing a plugin with an LLM provider.
#[derive(Debug, Clone)]
pub struct ReviewReport {
    /// Name of the reviewed plugin (derived from directory name).
    pub plugin_name: String,
    /// Provider used for the review (e.g., "anthropic").
    pub provider_name: String,
    /// Model used for the review (e.g., "claude-sonnet-4-20250514").
    pub model_name: String,
    /// The full review text returned by the LLM.
    pub review_text: String,
}

/// Review a plugin using an LLM provider.
///
/// Reads plugin files from `plugin_path`, constructs a structured review
/// prompt, sends it to `provider`, and returns the review report. If
/// `output_dir` is specified, the report is also saved as a Markdown file.
///
/// # Errors
///
/// Returns `SoukError::Io` if the required `plugin.json` cannot be read, or
/// `SoukError::LlmApiError` if the LLM provider call fails.
pub fn review_plugin(
    plugin_path: &Path,
    provider: &dyn LlmProvider,
    output_dir: Option<&Path>,
) -> Result<ReviewReport, SoukError> {
    // 1. Read plugin.json (required)
    let plugin_json_path = plugin_path.join(".claude-plugin").join("plugin.json");
    let plugin_json = std::fs::read_to_string(&plugin_json_path)?;

    // 2. Read extends-plugin.json (optional)
    let extends_path = plugin_path
        .join(".claude-plugin")
        .join("extends-plugin.json");
    let extends_json = std::fs::read_to_string(&extends_path).ok();

    // 3. Read README.md (optional)
    let readme_path = plugin_path.join("README.md");
    let readme = std::fs::read_to_string(&readme_path).ok();

    // 4. Enumerate skills
    let skills = enumerate_skills(plugin_path);
    let skills_summary: Vec<String> = skills
        .iter()
        .map(|s| format!("- {} (dir: {})", s.display_name, s.dir_name))
        .collect();

    // 5. Build the prompt
    let prompt = build_plugin_review_prompt(
        &plugin_json,
        extends_json.as_deref(),
        readme.as_deref(),
        &skills_summary,
    );

    // 6. Send to LLM
    let review_text = provider.complete(&prompt)?;

    // 7. Build report
    let plugin_name = plugin_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let report = ReviewReport {
        plugin_name: plugin_name.clone(),
        provider_name: provider.name().to_string(),
        model_name: provider.model().to_string(),
        review_text: review_text.clone(),
    };

    // 8. Save report if output_dir specified
    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)?;
        let report_path = dir.join(format!("{plugin_name}-review-report.md"));
        let report_content = format!(
            "# Plugin Review: {plugin_name}\n\n\
             **Provider:** {} ({})\n\
             **Date:** {}\n\n\
             ---\n\n\
             {review_text}\n",
            report.provider_name,
            report.model_name,
            current_date_string(),
        );
        std::fs::write(&report_path, report_content)?;
    }

    Ok(report)
}

/// Build the structured review prompt from plugin content.
///
/// This is intentionally kept as a pure function (no I/O) so it can be
/// unit-tested independently.
pub fn build_plugin_review_prompt(
    plugin_json: &str,
    extends_json: Option<&str>,
    readme: Option<&str>,
    skills: &[String],
) -> String {
    let mut prompt = String::with_capacity(2048);

    prompt.push_str(
        "You are a senior code reviewer specializing in Claude Code plugins. \
         Review this plugin for quality, security, and best practices.\n\n",
    );

    prompt.push_str("## plugin.json\n```json\n");
    prompt.push_str(plugin_json);
    prompt.push_str("\n```\n\n");

    if let Some(extends) = extends_json {
        prompt.push_str("## extends-plugin.json\n```json\n");
        prompt.push_str(extends);
        prompt.push_str("\n```\n\n");
    }

    if let Some(readme) = readme {
        prompt.push_str("## README.md\n");
        prompt.push_str(readme);
        prompt.push_str("\n\n");
    }

    if !skills.is_empty() {
        prompt.push_str("## Skills\n");
        for skill in skills {
            prompt.push_str(skill);
            prompt.push('\n');
        }
        prompt.push('\n');
    }

    prompt.push_str(
        "Please provide:\n\
         1. Executive Summary\n\
         2. Component Analysis (agents, skills, commands, hooks, MCP servers)\n\
         3. Code Quality Assessment\n\
         4. Documentation Review\n\
         5. Security Considerations\n\
         6. Recommendations (critical issues, suggested improvements, optional enhancements)\n\
         7. Overall Rating (1-10)\n",
    );

    prompt
}

/// Returns the current date as a `YYYY-MM-DD` string.
///
/// Uses `std::time::SystemTime` to avoid pulling in the `chrono` crate.
fn current_date_string() -> String {
    let now = std::time::SystemTime::now();
    let since_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = since_epoch.as_secs();
    // 86400 seconds per day; epoch is 1970-01-01.
    let days = secs / 86400;

    // Compute year/month/day from days since epoch (civil calendar).
    let (year, month, day) = days_to_civil(days as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Convert days since Unix epoch to (year, month, day).
///
/// Algorithm from Howard Hinnant's `chrono`-compatible date library.
fn days_to_civil(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::provider::MockProvider;
    use tempfile::TempDir;

    /// Create a minimal plugin directory with the required plugin.json.
    fn setup_plugin(tmp: &TempDir) -> std::path::PathBuf {
        let plugin = tmp.path().join("test-plugin");
        let claude_dir = plugin.join(".claude-plugin");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("plugin.json"),
            r#"{"name": "test-plugin", "version": "1.0.0", "description": "A test plugin"}"#,
        )
        .unwrap();
        plugin
    }

    /// Create a full plugin directory with extends, README, and skills.
    fn setup_full_plugin(tmp: &TempDir) -> std::path::PathBuf {
        let plugin = setup_plugin(tmp);

        // extends-plugin.json
        std::fs::write(
            plugin.join(".claude-plugin").join("extends-plugin.json"),
            r#"{"dependencies": {"some-dep": "^1.0.0"}}"#,
        )
        .unwrap();

        // README.md
        std::fs::write(
            plugin.join("README.md"),
            "# Test Plugin\n\nA plugin for testing.",
        )
        .unwrap();

        // Skills
        let skill_dir = plugin.join("skills").join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: My Skill\ndescription: Does things\n---\n# My Skill",
        )
        .unwrap();

        plugin
    }

    #[test]
    fn review_plugin_builds_report() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_full_plugin(&tmp);
        let provider = MockProvider::new("Great plugin! Rating: 9/10");

        let report = review_plugin(&plugin, &provider, None).unwrap();

        assert_eq!(report.plugin_name, "test-plugin");
        assert_eq!(report.provider_name, "mock");
        assert_eq!(report.model_name, "mock-model");
        assert_eq!(report.review_text, "Great plugin! Rating: 9/10");
    }

    #[test]
    fn review_plugin_saves_report_to_output_dir() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_full_plugin(&tmp);
        let output_dir = tmp.path().join("output");
        let provider = MockProvider::new("Looks good!");

        let report = review_plugin(&plugin, &provider, Some(&output_dir)).unwrap();

        assert_eq!(report.plugin_name, "test-plugin");

        let report_path = output_dir.join("test-plugin-review-report.md");
        assert!(report_path.exists(), "Report file should be created");

        let content = std::fs::read_to_string(&report_path).unwrap();
        assert!(content.contains("# Plugin Review: test-plugin"));
        assert!(content.contains("**Provider:** mock (mock-model)"));
        assert!(content.contains("Looks good!"));
    }

    #[test]
    fn review_plugin_minimal_plugin_no_extras() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin(&tmp);
        let provider = MockProvider::new("Minimal but valid.");

        let report = review_plugin(&plugin, &provider, None).unwrap();

        assert_eq!(report.plugin_name, "test-plugin");
        assert_eq!(report.review_text, "Minimal but valid.");
    }

    #[test]
    fn review_plugin_missing_plugin_json_returns_error() {
        let tmp = TempDir::new().unwrap();
        let plugin = tmp.path().join("no-plugin");
        std::fs::create_dir_all(&plugin).unwrap();
        let provider = MockProvider::new("should not reach");

        let result = review_plugin(&plugin, &provider, None);
        assert!(result.is_err());
    }

    #[test]
    fn build_prompt_contains_plugin_json() {
        let prompt = build_plugin_review_prompt(r#"{"name": "foo"}"#, None, None, &[]);
        assert!(prompt.contains("## plugin.json"));
        assert!(prompt.contains(r#"{"name": "foo"}"#));
    }

    #[test]
    fn build_prompt_includes_extends_when_present() {
        let prompt = build_plugin_review_prompt(
            r#"{"name": "foo"}"#,
            Some(r#"{"dependencies": {}}"#),
            None,
            &[],
        );
        assert!(prompt.contains("## extends-plugin.json"));
        assert!(prompt.contains(r#"{"dependencies": {}}"#));
    }

    #[test]
    fn build_prompt_includes_readme_when_present() {
        let prompt = build_plugin_review_prompt(
            r#"{"name": "foo"}"#,
            None,
            Some("# My Plugin\n\nHello world."),
            &[],
        );
        assert!(prompt.contains("## README.md"));
        assert!(prompt.contains("Hello world."));
    }

    #[test]
    fn build_prompt_includes_skills_when_present() {
        let skills = vec![
            "- commit-message (dir: git-commit)".to_string(),
            "- code-review (dir: code-review)".to_string(),
        ];
        let prompt = build_plugin_review_prompt(r#"{"name": "foo"}"#, None, None, &skills);
        assert!(prompt.contains("## Skills"));
        assert!(prompt.contains("commit-message"));
        assert!(prompt.contains("code-review"));
    }

    #[test]
    fn build_prompt_omits_optional_sections_when_absent() {
        let prompt = build_plugin_review_prompt(r#"{"name": "foo"}"#, None, None, &[]);
        assert!(!prompt.contains("## extends-plugin.json"));
        assert!(!prompt.contains("## README.md"));
        assert!(!prompt.contains("## Skills"));
    }

    #[test]
    fn build_prompt_requests_all_review_sections() {
        let prompt = build_plugin_review_prompt(r#"{"name": "foo"}"#, None, None, &[]);
        assert!(prompt.contains("Executive Summary"));
        assert!(prompt.contains("Component Analysis"));
        assert!(prompt.contains("Code Quality Assessment"));
        assert!(prompt.contains("Documentation Review"));
        assert!(prompt.contains("Security Considerations"));
        assert!(prompt.contains("Recommendations"));
        assert!(prompt.contains("Overall Rating (1-10)"));
    }

    #[test]
    fn current_date_string_has_correct_format() {
        let date = current_date_string();
        // Format: YYYY-MM-DD
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
        // Year should be plausible (2020+)
        let year: i32 = date[..4].parse().unwrap();
        assert!(year >= 2020);
    }
}
