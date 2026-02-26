/// Metadata extracted from a SKILL.md frontmatter.
#[derive(Debug, Clone)]
pub struct SkillMetadata {
    pub dir_name: String,
    pub display_name: String,
    pub path: std::path::PathBuf,
}

/// Parse the `name:` field from YAML frontmatter in SKILL.md content.
pub fn parse_skill_name_from_frontmatter(content: &str) -> Option<String> {
    let mut lines = content.lines();

    if lines.next()?.trim() != "---" {
        return None;
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("name:") {
            let name = rest.trim().trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_frontmatter_name() {
        let content = "---\nname: commit-message\ndescription: test\n---\n# Content";
        assert_eq!(
            parse_skill_name_from_frontmatter(content),
            Some("commit-message".to_string())
        );
    }

    #[test]
    fn parse_frontmatter_quoted_name() {
        let content = "---\nname: \"my skill\"\n---\n";
        assert_eq!(
            parse_skill_name_from_frontmatter(content),
            Some("my skill".to_string())
        );
    }

    #[test]
    fn no_frontmatter() {
        let content = "# Just a heading\nNo frontmatter here.";
        assert_eq!(parse_skill_name_from_frontmatter(content), None);
    }

    #[test]
    fn frontmatter_without_name() {
        let content = "---\ndescription: test\n---\n";
        assert_eq!(parse_skill_name_from_frontmatter(content), None);
    }

    #[test]
    fn empty_name_returns_none() {
        let content = "---\nname: \n---\n";
        assert_eq!(parse_skill_name_from_frontmatter(content), None);
    }
}
