//! CI workflow installation for various CI/CD providers.
//!
//! Detects which CI provider is in use (GitHub Actions, CircleCI, GitLab CI,
//! Buildkite, or compatible providers like Blacksmith and Northflank) and
//! generates the appropriate workflow configuration to run `souk validate marketplace`.

use std::fs;
use std::path::Path;

use crate::error::SoukError;

/// Supported CI providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiProvider {
    /// GitHub Actions (`.github/workflows/`)
    GitHub,
    /// Blacksmith (GitHub-compatible, uses `.github/workflows/`)
    Blacksmith,
    /// Northflank (GitHub-compatible, uses `.github/workflows/`)
    Northflank,
    /// CircleCI (`.circleci/`)
    CircleCi,
    /// GitLab CI (`.gitlab-ci.yml`)
    GitLab,
    /// Buildkite (`.buildkite/`)
    Buildkite,
}

impl CiProvider {
    /// Returns the lowercase name of the CI provider.
    pub fn name(&self) -> &str {
        match self {
            CiProvider::GitHub => "github",
            CiProvider::Blacksmith => "blacksmith",
            CiProvider::Northflank => "northflank",
            CiProvider::CircleCi => "circleci",
            CiProvider::GitLab => "gitlab",
            CiProvider::Buildkite => "buildkite",
        }
    }
}

impl std::fmt::Display for CiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Auto-detect the CI provider in use at the given project root.
///
/// Checks for configuration directories/files in priority order:
/// 1. `.github/workflows/` directory
/// 2. `.circleci/` directory
/// 3. `.gitlab-ci.yml` file
/// 4. `.buildkite/` directory
///
/// Returns `None` if no CI provider is detected.
pub fn detect_ci_provider(project_root: &Path) -> Option<CiProvider> {
    if project_root.join(".github").join("workflows").is_dir() {
        Some(CiProvider::GitHub)
    } else if project_root.join(".circleci").is_dir() {
        Some(CiProvider::CircleCi)
    } else if project_root.join(".gitlab-ci.yml").exists() {
        Some(CiProvider::GitLab)
    } else if project_root.join(".buildkite").is_dir() {
        Some(CiProvider::Buildkite)
    } else {
        None
    }
}

/// Install a CI workflow for the specified provider.
///
/// Creates the appropriate workflow configuration file.
/// Returns a human-readable description of what was done.
pub fn install_workflow(project_root: &Path, provider: &CiProvider) -> Result<String, SoukError> {
    match provider {
        CiProvider::GitHub | CiProvider::Blacksmith | CiProvider::Northflank => {
            install_github_workflow(project_root)
        }
        CiProvider::CircleCi => install_circleci_config(project_root),
        CiProvider::GitLab => install_gitlab_config(project_root),
        CiProvider::Buildkite => install_buildkite_config(project_root),
    }
}

/// GitHub Actions workflow template.
const GITHUB_WORKFLOW: &str = r#"name: Souk Marketplace Validation

on:
  push:
    paths:
      - '.claude-plugin/**'
      - 'plugins/**'
  pull_request:
    paths:
      - '.claude-plugin/**'
      - 'plugins/**'

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install souk
        run: cargo install souk

      - name: Validate marketplace
        run: souk validate marketplace
"#;

/// Install a GitHub Actions workflow file.
fn install_github_workflow(project_root: &Path) -> Result<String, SoukError> {
    let workflows_dir = project_root.join(".github").join("workflows");
    fs::create_dir_all(&workflows_dir)?;

    let workflow_path = workflows_dir.join("souk-validate.yml");

    if workflow_path.exists() {
        let existing = fs::read_to_string(&workflow_path)?;
        if existing.contains("souk validate marketplace") {
            return Ok(format!(
                "GitHub Actions workflow already exists at {}",
                workflow_path.display()
            ));
        }
    }

    fs::write(&workflow_path, GITHUB_WORKFLOW)?;

    Ok(format!(
        "Created GitHub Actions workflow at {}",
        workflow_path.display()
    ))
}

/// CircleCI configuration template.
const CIRCLECI_CONFIG: &str = r#"version: 2.1

jobs:
  souk-validate:
    docker:
      - image: cimg/rust:1.80
    steps:
      - checkout
      - run:
          name: Install souk
          command: cargo install souk
      - run:
          name: Validate marketplace
          command: souk validate marketplace

workflows:
  validate:
    jobs:
      - souk-validate:
          filters:
            branches:
              only: /.*/
"#;

/// Install a CircleCI configuration file.
///
/// If `.circleci/config.yml` already exists, appends the souk job as a comment
/// to avoid overwriting existing configuration.
fn install_circleci_config(project_root: &Path) -> Result<String, SoukError> {
    let circleci_dir = project_root.join(".circleci");
    fs::create_dir_all(&circleci_dir)?;

    let config_path = circleci_dir.join("config.yml");

    if config_path.exists() {
        let existing = fs::read_to_string(&config_path)?;
        if existing.contains("souk-validate") {
            return Ok(format!(
                "CircleCI souk-validate job already exists in {}",
                config_path.display()
            ));
        }

        // Append as commented section to not break existing config
        let snippet = "\n# --- Souk validation (merge into your config) ---\n\
             # Add the following job to your existing workflows:\n\
             #\n\
             # jobs:\n\
             #   souk-validate:\n\
             #     docker:\n\
             #       - image: cimg/rust:1.80\n\
             #     steps:\n\
             #       - checkout\n\
             #       - run:\n\
             #           name: Install souk\n\
             #           command: cargo install souk\n\
             #       - run:\n\
             #           name: Validate marketplace\n\
             #           command: souk validate marketplace\n";
        let new_content = format!("{existing}{snippet}");
        fs::write(&config_path, new_content)?;

        return Ok(format!(
            "Appended souk validation job (commented) to {}. \
             Please merge into your existing CircleCI configuration.",
            config_path.display()
        ));
    }

    fs::write(&config_path, CIRCLECI_CONFIG)?;

    Ok(format!(
        "Created CircleCI configuration at {}",
        config_path.display()
    ))
}

/// GitLab CI configuration template.
const GITLAB_CONFIG: &str = r#"souk-validate:
  stage: test
  image: rust:1.80
  script:
    - cargo install souk
    - souk validate marketplace
  rules:
    - changes:
        - .claude-plugin/**/*
        - plugins/**/*
"#;

/// Install a GitLab CI configuration.
///
/// If `.gitlab-ci.yml` already exists, appends the souk job.
fn install_gitlab_config(project_root: &Path) -> Result<String, SoukError> {
    let config_path = project_root.join(".gitlab-ci.yml");

    if config_path.exists() {
        let existing = fs::read_to_string(&config_path)?;
        if existing.contains("souk-validate") {
            return Ok(format!(
                "GitLab CI souk-validate job already exists in {}",
                config_path.display()
            ));
        }

        let new_content = format!("{existing}\n{GITLAB_CONFIG}");
        fs::write(&config_path, new_content)?;

        return Ok(format!(
            "Appended souk-validate job to {}",
            config_path.display()
        ));
    }

    fs::write(&config_path, GITLAB_CONFIG)?;

    Ok(format!(
        "Created GitLab CI configuration at {}",
        config_path.display()
    ))
}

/// Buildkite pipeline template.
const BUILDKITE_PIPELINE: &str = r#"steps:
  - label: ":souk: Validate Marketplace"
    command:
      - cargo install souk
      - souk validate marketplace
    agents:
      queue: default
"#;

/// Install a Buildkite pipeline configuration.
fn install_buildkite_config(project_root: &Path) -> Result<String, SoukError> {
    let buildkite_dir = project_root.join(".buildkite");
    fs::create_dir_all(&buildkite_dir)?;

    let pipeline_path = buildkite_dir.join("pipeline.yml");

    if pipeline_path.exists() {
        let existing = fs::read_to_string(&pipeline_path)?;
        if existing.contains("souk validate marketplace") || existing.contains("Validate Marketplace") {
            return Ok(format!(
                "Buildkite souk validation step already exists in {}",
                pipeline_path.display()
            ));
        }
    }

    fs::write(&pipeline_path, BUILDKITE_PIPELINE)?;

    Ok(format!(
        "Created Buildkite pipeline at {}",
        pipeline_path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn detect_ci_provider_finds_github_workflows() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".github/workflows")).unwrap();
        assert_eq!(detect_ci_provider(tmp.path()), Some(CiProvider::GitHub));
    }

    #[test]
    fn detect_ci_provider_finds_circleci() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".circleci")).unwrap();
        assert_eq!(detect_ci_provider(tmp.path()), Some(CiProvider::CircleCi));
    }

    #[test]
    fn detect_ci_provider_finds_gitlab() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".gitlab-ci.yml"), "").unwrap();
        assert_eq!(detect_ci_provider(tmp.path()), Some(CiProvider::GitLab));
    }

    #[test]
    fn detect_ci_provider_finds_buildkite() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".buildkite")).unwrap();
        assert_eq!(detect_ci_provider(tmp.path()), Some(CiProvider::Buildkite));
    }

    #[test]
    fn detect_ci_provider_returns_none_for_empty_dir() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(detect_ci_provider(tmp.path()), None);
    }

    #[test]
    fn install_github_workflow_creates_workflow_file() {
        let tmp = TempDir::new().unwrap();

        let result = install_github_workflow(tmp.path()).unwrap();
        assert!(result.contains("Created GitHub Actions workflow"));

        let workflow_path = tmp.path().join(".github/workflows/souk-validate.yml");
        assert!(workflow_path.exists());

        let content = fs::read_to_string(&workflow_path).unwrap();
        assert!(content.contains("Souk Marketplace Validation"));
        assert!(content.contains("souk validate marketplace"));
        assert!(content.contains("actions/checkout@v4"));
        assert!(content.contains("cargo install souk"));
    }

    #[test]
    fn install_github_workflow_skips_if_exists() {
        let tmp = TempDir::new().unwrap();
        let workflows_dir = tmp.path().join(".github/workflows");
        fs::create_dir_all(&workflows_dir).unwrap();
        fs::write(
            workflows_dir.join("souk-validate.yml"),
            "name: existing\nrun: souk validate marketplace\n",
        )
        .unwrap();

        let result = install_github_workflow(tmp.path()).unwrap();
        assert!(result.contains("already exists"));
    }

    #[test]
    fn install_circleci_config_creates_config_file() {
        let tmp = TempDir::new().unwrap();

        let result = install_circleci_config(tmp.path()).unwrap();
        assert!(result.contains("Created CircleCI configuration"));

        let config_path = tmp.path().join(".circleci/config.yml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("souk-validate"));
        assert!(content.contains("cargo install souk"));
        assert!(content.contains("souk validate marketplace"));
    }

    #[test]
    fn install_circleci_config_appends_to_existing() {
        let tmp = TempDir::new().unwrap();
        let circleci_dir = tmp.path().join(".circleci");
        fs::create_dir(&circleci_dir).unwrap();
        fs::write(
            circleci_dir.join("config.yml"),
            "version: 2.1\njobs:\n  build:\n    steps: []\n",
        )
        .unwrap();

        let result = install_circleci_config(tmp.path()).unwrap();
        assert!(result.contains("Appended"));

        let content = fs::read_to_string(circleci_dir.join("config.yml")).unwrap();
        assert!(content.contains("version: 2.1"));
        assert!(content.contains("Souk validation"));
    }

    #[test]
    fn install_gitlab_config_creates_file() {
        let tmp = TempDir::new().unwrap();

        let result = install_gitlab_config(tmp.path()).unwrap();
        assert!(result.contains("Created GitLab CI configuration"));

        let config_path = tmp.path().join(".gitlab-ci.yml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("souk-validate"));
        assert!(content.contains("souk validate marketplace"));
    }

    #[test]
    fn install_gitlab_config_appends_to_existing() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".gitlab-ci.yml"),
            "stages:\n  - test\n\nbuild:\n  script: echo ok\n",
        )
        .unwrap();

        let result = install_gitlab_config(tmp.path()).unwrap();
        assert!(result.contains("Appended"));

        let content = fs::read_to_string(tmp.path().join(".gitlab-ci.yml")).unwrap();
        assert!(content.contains("stages:"));
        assert!(content.contains("souk-validate"));
    }

    #[test]
    fn install_buildkite_config_creates_pipeline() {
        let tmp = TempDir::new().unwrap();

        let result = install_buildkite_config(tmp.path()).unwrap();
        assert!(result.contains("Created Buildkite pipeline"));

        let pipeline_path = tmp.path().join(".buildkite/pipeline.yml");
        assert!(pipeline_path.exists());

        let content = fs::read_to_string(&pipeline_path).unwrap();
        assert!(content.contains("Validate Marketplace"));
        assert!(content.contains("souk validate marketplace"));
    }

    #[test]
    fn ci_provider_name_returns_expected_values() {
        assert_eq!(CiProvider::GitHub.name(), "github");
        assert_eq!(CiProvider::Blacksmith.name(), "blacksmith");
        assert_eq!(CiProvider::Northflank.name(), "northflank");
        assert_eq!(CiProvider::CircleCi.name(), "circleci");
        assert_eq!(CiProvider::GitLab.name(), "gitlab");
        assert_eq!(CiProvider::Buildkite.name(), "buildkite");
    }

    #[test]
    fn ci_provider_display() {
        assert_eq!(format!("{}", CiProvider::GitHub), "github");
        assert_eq!(format!("{}", CiProvider::CircleCi), "circleci");
    }

    #[test]
    fn install_workflow_dispatches_to_github_for_compatible_providers() {
        let tmp = TempDir::new().unwrap();

        // All three should create the same GitHub workflow file
        for provider in &[CiProvider::GitHub, CiProvider::Blacksmith, CiProvider::Northflank] {
            let tmp_inner = TempDir::new().unwrap();
            let result = install_workflow(tmp_inner.path(), provider).unwrap();
            assert!(result.contains("GitHub Actions workflow"));

            let workflow_path = tmp_inner.path().join(".github/workflows/souk-validate.yml");
            assert!(workflow_path.exists());
        }

        // Clean up unused tmp binding
        drop(tmp);
    }

    #[test]
    fn detect_ci_provider_github_takes_priority_over_others() {
        let tmp = TempDir::new().unwrap();
        // Create both GitHub and GitLab configs
        fs::create_dir_all(tmp.path().join(".github/workflows")).unwrap();
        fs::write(tmp.path().join(".gitlab-ci.yml"), "").unwrap();

        // GitHub should win since it's checked first
        assert_eq!(detect_ci_provider(tmp.path()), Some(CiProvider::GitHub));
    }
}
