//! LLM-powered skill review for Claude Code plugins.
//!
//! Provides [`review_skills`] which sends skill content to an LLM provider
//! and produces structured review reports. Supports reviewing individual skills
//! by name, or all skills in a plugin at once.

use std::path::Path;

use crate::error::SoukError;
use crate::resolution::skill::enumerate_skills;
use crate::review::provider::LlmProvider;
use crate::types::skill::SkillMetadata;

/// The result of reviewing a single skill via an LLM provider.
#[derive(Debug, Clone)]
pub struct SkillReviewReport {
    /// The human-readable skill name (from SKILL.md frontmatter or directory name).
    pub skill_name: String,
    /// The directory name of the skill under `skills/`.
    pub skill_dir: String,
    /// The LLM provider used (e.g. "anthropic", "openai", "mock").
    pub provider_name: String,
    /// The model identifier used (e.g. "claude-sonnet-4-20250514").
    pub model_name: String,
    /// The full review text returned by the LLM.
    pub review_text: String,
}

/// Review selected skills in a plugin using an LLM provider.
///
/// # Skill selection
///
/// - If `all` is `true`, every skill found in the plugin is reviewed.
/// - If `skill_names` contains one or more names, only those skills are
///   reviewed. Each name is matched against the directory name **or** the
///   display name extracted from SKILL.md frontmatter.
/// - If `skill_names` is empty and `all` is `false`, an error is returned
///   listing the available skills (suitable for the CLI to present an
///   interactive selection menu).
///
/// # Output
///
/// When `output_dir` is provided, a Markdown report file is written for each
/// reviewed skill at `<output_dir>/<skill-dir-name>-skill-review.md`.
///
/// Returns a [`SkillReviewReport`] for every successfully reviewed skill.
///
/// # Errors
///
/// - [`SoukError::Other`] if the plugin contains no skills.
/// - [`SoukError::Other`] if no skills are specified and `all` is `false`.
/// - [`SoukError::SkillNotFound`] if a requested skill name cannot be resolved.
/// - [`SoukError::Io`] if SKILL.md cannot be read or reports cannot be written.
/// - [`SoukError::LlmApiError`] if the LLM provider call fails.
pub fn review_skills(
    plugin_path: &Path,
    skill_names: &[String],
    all: bool,
    provider: &dyn LlmProvider,
    output_dir: Option<&Path>,
) -> Result<Vec<SkillReviewReport>, SoukError> {
    let available = enumerate_skills(plugin_path);

    if available.is_empty() {
        return Err(SoukError::Other("No skills found in plugin".into()));
    }

    // Determine which skills to review.
    let skills_to_review: Vec<&SkillMetadata> = if all {
        available.iter().collect()
    } else if skill_names.is_empty() {
        // Return available skills for the caller to handle interactive selection.
        let listing = available
            .iter()
            .enumerate()
            .map(|(i, s)| format!("  {}. {} ({})", i + 1, s.display_name, s.dir_name))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(SoukError::Other(format!(
            "No skills specified. Available skills:\n{listing}"
        )));
    } else {
        let mut resolved = Vec::new();
        for name in skill_names {
            if let Some(skill) = available
                .iter()
                .find(|s| s.dir_name == *name || s.display_name == *name)
            {
                resolved.push(skill);
            } else {
                let plugin_name = plugin_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                return Err(SoukError::SkillNotFound {
                    plugin: plugin_name,
                    skill: name.clone(),
                });
            }
        }
        resolved
    };

    let mut reports = Vec::new();

    for skill in &skills_to_review {
        let skill_md_path = skill.path.join("SKILL.md");
        let skill_content = std::fs::read_to_string(&skill_md_path).map_err(SoukError::Io)?;

        let prompt = build_skill_review_prompt(&skill.display_name, &skill_content);
        let review_text = provider.complete(&prompt)?;

        let report = SkillReviewReport {
            skill_name: skill.display_name.clone(),
            skill_dir: skill.dir_name.clone(),
            provider_name: provider.name().to_string(),
            model_name: provider.model().to_string(),
            review_text: review_text.clone(),
        };

        if let Some(dir) = output_dir {
            std::fs::create_dir_all(dir)?;
            let report_path = dir.join(format!("{}-skill-review.md", skill.dir_name));
            let content = format!(
                "# Skill Review: {}\n\n\
                 **Provider:** {} ({})\n\n\
                 ---\n\n\
                 {}\n",
                skill.display_name, report.provider_name, report.model_name, review_text,
            );
            std::fs::write(&report_path, content)?;
        }

        reports.push(report);
    }

    Ok(reports)
}

/// Build the LLM prompt for reviewing a single skill.
fn build_skill_review_prompt(skill_name: &str, skill_content: &str) -> String {
    format!(
        "You are a senior code reviewer. Review this Claude Code skill named \
         '{skill_name}' for quality, clarity, and effectiveness.\n\n\
         ## SKILL.md Content\n\
         ```markdown\n\
         {skill_content}\n\
         ```\n\n\
         Please provide:\n\
         1. Overall assessment of the skill's purpose and clarity\n\
         2. Quality of instructions and examples\n\
         3. Potential issues or ambiguities\n\
         4. Suggestions for improvement\n\
         5. Rating (1-10)\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::provider::MockProvider;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Create a plugin directory with two skills for testing.
    fn setup_plugin_with_skills(tmp: &TempDir) -> PathBuf {
        let plugin = tmp.path().join("test-plugin");
        let skills = plugin.join("skills");

        let commit = skills.join("git-commit");
        std::fs::create_dir_all(&commit).unwrap();
        std::fs::write(
            commit.join("SKILL.md"),
            "---\nname: commit-message\ndescription: Helps write commit messages\n---\n\
             # Commit Message Skill\n\nGenerate clear, conventional commit messages.",
        )
        .unwrap();

        let review = skills.join("code-review");
        std::fs::create_dir_all(&review).unwrap();
        std::fs::write(
            review.join("SKILL.md"),
            "# Code Review Skill\n\nReview code for quality and correctness.",
        )
        .unwrap();

        plugin
    }

    /// Create a plugin directory with no skills.
    fn setup_plugin_without_skills(tmp: &TempDir) -> PathBuf {
        let plugin = tmp.path().join("empty-plugin");
        std::fs::create_dir_all(&plugin).unwrap();
        plugin
    }

    #[test]
    fn review_all_skills_reviews_every_skill() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let provider = MockProvider::new("Looks good! Rating: 8/10");

        let reports = review_skills(&plugin, &[], true, &provider, None).unwrap();

        assert_eq!(reports.len(), 2);

        // enumerate_skills sorts by directory name, so code-review comes first.
        assert_eq!(reports[0].skill_dir, "code-review");
        assert_eq!(reports[0].skill_name, "code-review"); // no frontmatter name
        assert_eq!(reports[0].provider_name, "mock");
        assert_eq!(reports[0].model_name, "mock-model");
        assert_eq!(reports[0].review_text, "Looks good! Rating: 8/10");

        assert_eq!(reports[1].skill_dir, "git-commit");
        assert_eq!(reports[1].skill_name, "commit-message"); // from frontmatter
    }

    #[test]
    fn review_specific_skill_by_dir_name() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let provider = MockProvider::new("Excellent skill.");

        let names = vec!["code-review".to_string()];
        let reports = review_skills(&plugin, &names, false, &provider, None).unwrap();

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].skill_dir, "code-review");
        assert_eq!(reports[0].review_text, "Excellent skill.");
    }

    #[test]
    fn review_specific_skill_by_display_name() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let provider = MockProvider::new("Great commit skill.");

        let names = vec!["commit-message".to_string()];
        let reports = review_skills(&plugin, &names, false, &provider, None).unwrap();

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].skill_dir, "git-commit");
        assert_eq!(reports[0].skill_name, "commit-message");
    }

    #[test]
    fn review_nonexistent_skill_returns_error() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let provider = MockProvider::new("ignored");

        let names = vec!["nonexistent".to_string()];
        let result = review_skills(&plugin, &names, false, &provider, None);

        assert!(result.is_err());
        match result.unwrap_err() {
            SoukError::SkillNotFound { plugin, skill } => {
                assert_eq!(plugin, "test-plugin");
                assert_eq!(skill, "nonexistent");
            }
            other => panic!("Expected SkillNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn review_saves_reports_to_output_dir() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let output_dir = tmp.path().join("reviews");
        let provider = MockProvider::new("Review output here.");

        let reports =
            review_skills(&plugin, &[], true, &provider, Some(&output_dir)).unwrap();

        assert_eq!(reports.len(), 2);

        // Verify report files were created.
        let report1 = output_dir.join("code-review-skill-review.md");
        let report2 = output_dir.join("git-commit-skill-review.md");

        assert!(report1.is_file(), "Expected report file: {report1:?}");
        assert!(report2.is_file(), "Expected report file: {report2:?}");

        let content1 = std::fs::read_to_string(&report1).unwrap();
        assert!(content1.contains("# Skill Review: code-review"));
        assert!(content1.contains("**Provider:** mock (mock-model)"));
        assert!(content1.contains("Review output here."));

        let content2 = std::fs::read_to_string(&report2).unwrap();
        assert!(content2.contains("# Skill Review: commit-message"));
    }

    #[test]
    fn review_plugin_with_no_skills_returns_error() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_without_skills(&tmp);
        let provider = MockProvider::new("ignored");

        let result = review_skills(&plugin, &[], true, &provider, None);

        assert!(result.is_err());
        match result.unwrap_err() {
            SoukError::Other(msg) => {
                assert!(
                    msg.contains("No skills found"),
                    "Unexpected message: {msg}"
                );
            }
            other => panic!("Expected Other with 'No skills found', got: {other:?}"),
        }
    }

    #[test]
    fn review_no_skills_specified_and_not_all_lists_available() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let provider = MockProvider::new("ignored");

        let result = review_skills(&plugin, &[], false, &provider, None);

        assert!(result.is_err());
        match result.unwrap_err() {
            SoukError::Other(msg) => {
                assert!(
                    msg.contains("No skills specified"),
                    "Unexpected message: {msg}"
                );
                assert!(msg.contains("code-review"), "Should list code-review: {msg}");
                assert!(
                    msg.contains("commit-message"),
                    "Should list commit-message: {msg}"
                );
            }
            other => panic!("Expected Other with skill listing, got: {other:?}"),
        }
    }

    #[test]
    fn review_multiple_skills_by_name() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let provider = MockProvider::new("Reviewed.");

        let names = vec!["code-review".to_string(), "commit-message".to_string()];
        let reports = review_skills(&plugin, &names, false, &provider, None).unwrap();

        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].skill_dir, "code-review");
        assert_eq!(reports[1].skill_dir, "git-commit");
    }

    #[test]
    fn build_prompt_includes_skill_name_and_content() {
        let prompt = build_skill_review_prompt("my-skill", "# My Skill\nDoes things.");
        assert!(prompt.contains("'my-skill'"));
        assert!(prompt.contains("# My Skill"));
        assert!(prompt.contains("Does things."));
        assert!(prompt.contains("Rating (1-10)"));
    }
}
