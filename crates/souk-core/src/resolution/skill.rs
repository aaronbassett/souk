use std::path::{Path, PathBuf};

use crate::error::SoukError;
use crate::types::skill::{parse_skill_name_from_frontmatter, SkillMetadata};

pub fn resolve_skill(
    plugin_path: &Path,
    input: &str,
) -> Result<PathBuf, SoukError> {
    let plugin_name = plugin_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let direct = PathBuf::from(input);
    if direct.is_dir() && direct.join("SKILL.md").is_file() {
        return direct.canonicalize().map_err(SoukError::Io);
    }

    let skills_dir = plugin_path.join("skills").join(input);
    if skills_dir.is_dir() && skills_dir.join("SKILL.md").is_file() {
        return skills_dir.canonicalize().map_err(SoukError::Io);
    }

    let skills_parent = plugin_path.join("skills");
    if skills_parent.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&skills_parent) {
            for entry in entries.flatten() {
                let skill_md = entry.path().join("SKILL.md");
                if skill_md.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&skill_md) {
                        if let Some(name) = parse_skill_name_from_frontmatter(&content) {
                            if name == input {
                                return entry
                                    .path()
                                    .canonicalize()
                                    .map_err(SoukError::Io);
                            }
                        }
                    }
                }
            }
        }
    }

    Err(SoukError::SkillNotFound {
        plugin: plugin_name,
        skill: input.to_string(),
    })
}

pub fn enumerate_skills(plugin_path: &Path) -> Vec<SkillMetadata> {
    let skills_dir = plugin_path.join("skills");
    let mut skills = Vec::new();

    if !skills_dir.is_dir() {
        return skills;
    }

    let Ok(entries) = std::fs::read_dir(&skills_dir) else {
        return skills;
    };

    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let skill_md = entry.path().join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }

        let dir_name = entry.file_name().to_string_lossy().to_string();

        let display_name = std::fs::read_to_string(&skill_md)
            .ok()
            .and_then(|content| parse_skill_name_from_frontmatter(&content))
            .unwrap_or_else(|| dir_name.clone());

        skills.push(SkillMetadata {
            dir_name,
            display_name,
            path: entry.path(),
        });
    }

    skills
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_plugin_with_skills(tmp: &TempDir) -> PathBuf {
        let plugin = tmp.path().join("my-plugin");
        let skills = plugin.join("skills");

        let commit = skills.join("git-commit");
        std::fs::create_dir_all(&commit).unwrap();
        std::fs::write(
            commit.join("SKILL.md"),
            "---\nname: commit-message\ndescription: test\n---\n# Commit",
        )
        .unwrap();

        let review = skills.join("code-review");
        std::fs::create_dir_all(&review).unwrap();
        std::fs::write(review.join("SKILL.md"), "# Code Review\nNo frontmatter.").unwrap();

        plugin
    }

    #[test]
    fn resolve_by_dir_name() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let result = resolve_skill(&plugin, "git-commit");
        assert!(result.is_ok());
    }

    #[test]
    fn resolve_by_frontmatter_name() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let result = resolve_skill(&plugin, "commit-message");
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("git-commit"));
    }

    #[test]
    fn resolve_not_found() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let result = resolve_skill(&plugin, "nonexistent");
        assert!(matches!(result, Err(SoukError::SkillNotFound { .. })));
    }

    #[test]
    fn enumerate_returns_all_skills() {
        let tmp = TempDir::new().unwrap();
        let plugin = setup_plugin_with_skills(&tmp);
        let skills = enumerate_skills(&plugin);
        assert_eq!(skills.len(), 2);

        assert_eq!(skills[0].dir_name, "code-review");
        assert_eq!(skills[0].display_name, "code-review");

        assert_eq!(skills[1].dir_name, "git-commit");
        assert_eq!(skills[1].display_name, "commit-message");
    }

    #[test]
    fn enumerate_empty_skills() {
        let tmp = TempDir::new().unwrap();
        let plugin = tmp.path().join("no-skills");
        std::fs::create_dir_all(&plugin).unwrap();
        let skills = enumerate_skills(&plugin);
        assert!(skills.is_empty());
    }
}
