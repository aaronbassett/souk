//! Hook installation for various git hook managers.
//!
//! Detects which hook manager is in use (lefthook, husky, overcommit, hk,
//! simple-git-hooks, or native git hooks) and generates the appropriate
//! configuration to run `souk ci run` on pre-commit and pre-push.

use std::fs;
use std::path::Path;

use crate::error::SoukError;

/// Supported git hook managers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookManager {
    /// Native git hooks (`.git/hooks/`)
    Native,
    /// Lefthook (`lefthook.yml`)
    Lefthook,
    /// Husky (`.husky/`)
    Husky,
    /// Overcommit (`.overcommit.yml`)
    Overcommit,
    /// hk (`hk.toml`)
    Hk,
    /// simple-git-hooks (`.simple-git-hooks.json`)
    SimpleGitHooks,
}

impl HookManager {
    /// Returns the lowercase name of the hook manager.
    pub fn name(&self) -> &str {
        match self {
            HookManager::Native => "native",
            HookManager::Lefthook => "lefthook",
            HookManager::Husky => "husky",
            HookManager::Overcommit => "overcommit",
            HookManager::Hk => "hk",
            HookManager::SimpleGitHooks => "simple-git-hooks",
        }
    }
}

impl std::fmt::Display for HookManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Auto-detect the hook manager in use at the given project root.
///
/// Checks for configuration files in priority order:
/// 1. `lefthook.yml` or `lefthook.yaml`
/// 2. `.husky/` directory
/// 3. `.overcommit.yml`
/// 4. `hk.toml`
/// 5. `.simple-git-hooks.json`
///
/// Returns `None` if no hook manager is detected (caller should default to native).
pub fn detect_hook_manager(project_root: &Path) -> Option<HookManager> {
    if project_root.join("lefthook.yml").exists() || project_root.join("lefthook.yaml").exists() {
        Some(HookManager::Lefthook)
    } else if project_root.join(".husky").is_dir() {
        Some(HookManager::Husky)
    } else if project_root.join(".overcommit.yml").exists() {
        Some(HookManager::Overcommit)
    } else if project_root.join("hk.toml").exists() {
        Some(HookManager::Hk)
    } else if project_root.join(".simple-git-hooks.json").exists() {
        Some(HookManager::SimpleGitHooks)
    } else {
        None
    }
}

/// Install git hooks for the specified hook manager.
///
/// Creates or appends configuration files appropriate for the manager.
/// Returns a human-readable description of what was done.
pub fn install_hooks(project_root: &Path, manager: &HookManager) -> Result<String, SoukError> {
    match manager {
        HookManager::Native => install_native_hooks(project_root),
        HookManager::Lefthook => install_lefthook(project_root),
        HookManager::Husky => install_husky(project_root),
        HookManager::Overcommit => install_overcommit(project_root),
        HookManager::Hk => install_hk(project_root),
        HookManager::SimpleGitHooks => install_simple_git_hooks(project_root),
    }
}

/// The shebang and hook body for native git hooks.
const NATIVE_HOOK_TEMPLATE: &str = "#!/bin/sh\nsouk ci run {hook}\n";

/// Install native git hooks by writing scripts to `.git/hooks/`.
fn install_native_hooks(project_root: &Path) -> Result<String, SoukError> {
    let hooks_dir = project_root.join(".git").join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    let mut actions = Vec::new();

    for hook_name in &["pre-commit", "pre-push"] {
        let hook_path = hooks_dir.join(hook_name);
        let content = NATIVE_HOOK_TEMPLATE.replace("{hook}", hook_name);
        fs::write(&hook_path, &content)?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&hook_path, perms)?;
        }

        actions.push(format!("Created {}", hook_path.display()));
    }

    Ok(format!(
        "Installed native git hooks:\n  {}",
        actions.join("\n  ")
    ))
}

/// YAML snippet to append to `lefthook.yml`.
const LEFTHOOK_SNIPPET: &str = r#"
pre-commit:
  commands:
    souk-validate:
      run: souk ci run pre-commit

pre-push:
  commands:
    souk-validate:
      run: souk ci run pre-push
"#;

/// Install hooks by appending configuration to `lefthook.yml`.
fn install_lefthook(project_root: &Path) -> Result<String, SoukError> {
    let config_path = if project_root.join("lefthook.yml").exists() {
        project_root.join("lefthook.yml")
    } else if project_root.join("lefthook.yaml").exists() {
        project_root.join("lefthook.yaml")
    } else {
        // Create a new lefthook.yml
        project_root.join("lefthook.yml")
    };

    let existing = if config_path.exists() {
        fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    // Check if souk hooks are already configured
    if existing.contains("souk-validate") {
        return Ok(format!(
            "Lefthook hooks already configured in {}",
            config_path.display()
        ));
    }

    let new_content = format!("{existing}{LEFTHOOK_SNIPPET}");
    fs::write(&config_path, new_content)?;

    Ok(format!(
        "Appended souk hooks to {}",
        config_path.display()
    ))
}

/// Husky hook script content (no shebang needed for Husky v9+).
const HUSKY_HOOK_TEMPLATE: &str = "souk ci run {hook}\n";

/// Install hooks by writing scripts into the `.husky/` directory.
fn install_husky(project_root: &Path) -> Result<String, SoukError> {
    let husky_dir = project_root.join(".husky");
    fs::create_dir_all(&husky_dir)?;

    let mut actions = Vec::new();

    for hook_name in &["pre-commit", "pre-push"] {
        let hook_path = husky_dir.join(hook_name);
        let content = HUSKY_HOOK_TEMPLATE.replace("{hook}", hook_name);

        // If file already exists, check if souk line is already there
        if hook_path.exists() {
            let existing = fs::read_to_string(&hook_path)?;
            if existing.contains("souk ci run") {
                actions.push(format!("Already configured: {}", hook_path.display()));
                continue;
            }
            // Append to existing hook
            let new_content = format!("{existing}\n{content}");
            fs::write(&hook_path, new_content)?;
            actions.push(format!("Appended to {}", hook_path.display()));
        } else {
            fs::write(&hook_path, &content)?;
            actions.push(format!("Created {}", hook_path.display()));
        }

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&hook_path, perms)?;
        }
    }

    Ok(format!(
        "Installed Husky hooks:\n  {}",
        actions.join("\n  ")
    ))
}

/// YAML snippet for overcommit.
const OVERCOMMIT_SNIPPET: &str = r#"
# Add the following to your .overcommit.yml:
#
# PreCommit:
#   SoukValidate:
#     enabled: true
#     command: ['souk', 'ci', 'run', 'pre-commit']
#
# PrePush:
#   SoukValidate:
#     enabled: true
#     command: ['souk', 'ci', 'run', 'pre-push']
"#;

/// Install hooks for overcommit by appending a commented note to `.overcommit.yml`.
///
/// Overcommit uses a structured YAML format that requires careful merging,
/// so we append configuration as a commented block for the user to integrate.
fn install_overcommit(project_root: &Path) -> Result<String, SoukError> {
    let config_path = project_root.join(".overcommit.yml");

    let existing = if config_path.exists() {
        fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    if existing.contains("SoukValidate") {
        return Ok(format!(
            "Overcommit hooks already configured in {}",
            config_path.display()
        ));
    }

    let new_content = format!("{existing}{OVERCOMMIT_SNIPPET}");
    fs::write(&config_path, new_content)?;

    Ok(format!(
        "Added souk hook configuration notes to {}. \
         Please integrate the commented YAML into your overcommit config.",
        config_path.display()
    ))
}

/// TOML snippet for hk.
const HK_SNIPPET: &str = r#"
# Add the following to your hk.toml:
#
# [hooks.pre-commit.souk-validate]
# run = "souk ci run pre-commit"
#
# [hooks.pre-push.souk-validate]
# run = "souk ci run pre-push"
"#;

/// Install hooks for hk by appending a commented note to `hk.toml`.
///
/// hk uses a structured TOML format that requires careful merging,
/// so we append configuration as a commented block for the user to integrate.
fn install_hk(project_root: &Path) -> Result<String, SoukError> {
    let config_path = project_root.join("hk.toml");

    let existing = if config_path.exists() {
        fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    if existing.contains("souk-validate") {
        return Ok(format!(
            "hk hooks already configured in {}",
            config_path.display()
        ));
    }

    let new_content = format!("{existing}{HK_SNIPPET}");
    fs::write(&config_path, new_content)?;

    Ok(format!(
        "Added souk hook configuration notes to {}. \
         Please integrate the commented TOML into your hk config.",
        config_path.display()
    ))
}

/// JSON snippet for simple-git-hooks.
const SIMPLE_GIT_HOOKS_NOTE: &str = r#"
Merge the following into your .simple-git-hooks.json:

{
  "pre-commit": "souk ci run pre-commit",
  "pre-push": "souk ci run pre-push"
}
"#;

/// Install hooks for simple-git-hooks by updating `.simple-git-hooks.json`.
///
/// If the file exists, we attempt to merge our hook entries. If the file
/// does not exist, we create it with the souk hooks.
fn install_simple_git_hooks(project_root: &Path) -> Result<String, SoukError> {
    let config_path = project_root.join(".simple-git-hooks.json");

    if config_path.exists() {
        let existing = fs::read_to_string(&config_path)?;
        if existing.contains("souk ci run") {
            return Ok(format!(
                "simple-git-hooks already configured in {}",
                config_path.display()
            ));
        }

        // Try to merge into existing JSON
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&existing);
        match parsed {
            Ok(serde_json::Value::Object(mut map)) => {
                map.entry("pre-commit")
                    .or_insert(serde_json::Value::String(
                        "souk ci run pre-commit".to_string(),
                    ));
                map.entry("pre-push")
                    .or_insert(serde_json::Value::String(
                        "souk ci run pre-push".to_string(),
                    ));
                let new_content = serde_json::to_string_pretty(&map)?;
                fs::write(&config_path, format!("{new_content}\n"))?;
                Ok(format!(
                    "Merged souk hooks into {}",
                    config_path.display()
                ))
            }
            _ => Ok(format!(
                "Could not parse {}. {SIMPLE_GIT_HOOKS_NOTE}",
                config_path.display()
            )),
        }
    } else {
        // Create new file
        let hooks = serde_json::json!({
            "pre-commit": "souk ci run pre-commit",
            "pre-push": "souk ci run pre-push"
        });
        let content = serde_json::to_string_pretty(&hooks)?;
        fs::write(&config_path, format!("{content}\n"))?;
        Ok(format!("Created {}", config_path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn detect_hook_manager_finds_lefthook_yml() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("lefthook.yml"), "").unwrap();
        assert_eq!(
            detect_hook_manager(tmp.path()),
            Some(HookManager::Lefthook)
        );
    }

    #[test]
    fn detect_hook_manager_finds_lefthook_yaml() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("lefthook.yaml"), "").unwrap();
        assert_eq!(
            detect_hook_manager(tmp.path()),
            Some(HookManager::Lefthook)
        );
    }

    #[test]
    fn detect_hook_manager_finds_husky() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".husky")).unwrap();
        assert_eq!(detect_hook_manager(tmp.path()), Some(HookManager::Husky));
    }

    #[test]
    fn detect_hook_manager_finds_overcommit() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".overcommit.yml"), "").unwrap();
        assert_eq!(
            detect_hook_manager(tmp.path()),
            Some(HookManager::Overcommit)
        );
    }

    #[test]
    fn detect_hook_manager_finds_hk() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("hk.toml"), "").unwrap();
        assert_eq!(detect_hook_manager(tmp.path()), Some(HookManager::Hk));
    }

    #[test]
    fn detect_hook_manager_finds_simple_git_hooks() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".simple-git-hooks.json"), "{}").unwrap();
        assert_eq!(
            detect_hook_manager(tmp.path()),
            Some(HookManager::SimpleGitHooks)
        );
    }

    #[test]
    fn detect_hook_manager_returns_none_for_empty_dir() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(detect_hook_manager(tmp.path()), None);
    }

    #[test]
    fn install_native_hooks_creates_hook_files() {
        let tmp = TempDir::new().unwrap();
        // Create .git directory to simulate a git repo
        fs::create_dir(tmp.path().join(".git")).unwrap();

        let result = install_native_hooks(tmp.path()).unwrap();
        assert!(result.contains("Installed native git hooks"));

        let pre_commit = tmp.path().join(".git/hooks/pre-commit");
        let pre_push = tmp.path().join(".git/hooks/pre-push");

        assert!(pre_commit.exists());
        assert!(pre_push.exists());

        let pre_commit_content = fs::read_to_string(&pre_commit).unwrap();
        assert!(pre_commit_content.contains("#!/bin/sh"));
        assert!(pre_commit_content.contains("souk ci run pre-commit"));

        let pre_push_content = fs::read_to_string(&pre_push).unwrap();
        assert!(pre_push_content.contains("souk ci run pre-push"));

        // Verify executable permission on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::metadata(&pre_commit).unwrap().permissions();
            assert!(perms.mode() & 0o111 != 0, "pre-commit should be executable");
        }
    }

    #[test]
    fn install_husky_creates_hook_files() {
        let tmp = TempDir::new().unwrap();

        let result = install_husky(tmp.path()).unwrap();
        assert!(result.contains("Installed Husky hooks"));

        let pre_commit = tmp.path().join(".husky/pre-commit");
        let pre_push = tmp.path().join(".husky/pre-push");

        assert!(pre_commit.exists());
        assert!(pre_push.exists());

        let pre_commit_content = fs::read_to_string(&pre_commit).unwrap();
        assert!(pre_commit_content.contains("souk ci run pre-commit"));

        let pre_push_content = fs::read_to_string(&pre_push).unwrap();
        assert!(pre_push_content.contains("souk ci run pre-push"));
    }

    #[test]
    fn install_lefthook_creates_config() {
        let tmp = TempDir::new().unwrap();

        let result = install_lefthook(tmp.path()).unwrap();
        assert!(result.contains("Appended souk hooks"));

        let config = fs::read_to_string(tmp.path().join("lefthook.yml")).unwrap();
        assert!(config.contains("souk-validate"));
        assert!(config.contains("souk ci run pre-commit"));
        assert!(config.contains("souk ci run pre-push"));
    }

    #[test]
    fn install_lefthook_appends_to_existing() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("lefthook.yml"),
            "# existing config\nsome-key: value\n",
        )
        .unwrap();

        let result = install_lefthook(tmp.path()).unwrap();
        assert!(result.contains("Appended souk hooks"));

        let config = fs::read_to_string(tmp.path().join("lefthook.yml")).unwrap();
        assert!(config.contains("# existing config"));
        assert!(config.contains("souk-validate"));
    }

    #[test]
    fn install_lefthook_skips_if_already_configured() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("lefthook.yml"),
            "pre-commit:\n  commands:\n    souk-validate:\n      run: souk ci run pre-commit\n",
        )
        .unwrap();

        let result = install_lefthook(tmp.path()).unwrap();
        assert!(result.contains("already configured"));
    }

    #[test]
    fn install_overcommit_appends_note() {
        let tmp = TempDir::new().unwrap();

        let result = install_overcommit(tmp.path()).unwrap();
        assert!(result.contains("Added souk hook configuration notes"));

        let config = fs::read_to_string(tmp.path().join(".overcommit.yml")).unwrap();
        assert!(config.contains("SoukValidate"));
    }

    #[test]
    fn install_hk_appends_note() {
        let tmp = TempDir::new().unwrap();

        let result = install_hk(tmp.path()).unwrap();
        assert!(result.contains("Added souk hook configuration notes"));

        let config = fs::read_to_string(tmp.path().join("hk.toml")).unwrap();
        assert!(config.contains("souk-validate"));
    }

    #[test]
    fn install_simple_git_hooks_creates_new_file() {
        let tmp = TempDir::new().unwrap();

        let result = install_simple_git_hooks(tmp.path()).unwrap();
        assert!(result.contains("Created"));

        let config = fs::read_to_string(tmp.path().join(".simple-git-hooks.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&config).unwrap();
        assert_eq!(parsed["pre-commit"], "souk ci run pre-commit");
        assert_eq!(parsed["pre-push"], "souk ci run pre-push");
    }

    #[test]
    fn install_simple_git_hooks_merges_into_existing() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".simple-git-hooks.json"),
            r#"{"commit-msg": "echo ok"}"#,
        )
        .unwrap();

        let result = install_simple_git_hooks(tmp.path()).unwrap();
        assert!(result.contains("Merged souk hooks"));

        let config = fs::read_to_string(tmp.path().join(".simple-git-hooks.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&config).unwrap();
        assert_eq!(parsed["pre-commit"], "souk ci run pre-commit");
        assert_eq!(parsed["pre-push"], "souk ci run pre-push");
        assert_eq!(parsed["commit-msg"], "echo ok");
    }

    #[test]
    fn install_husky_appends_to_existing_hooks() {
        let tmp = TempDir::new().unwrap();
        let husky_dir = tmp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();
        fs::write(husky_dir.join("pre-commit"), "echo 'existing hook'\n").unwrap();

        let result = install_husky(tmp.path()).unwrap();
        assert!(result.contains("Appended to"));

        let content = fs::read_to_string(husky_dir.join("pre-commit")).unwrap();
        assert!(content.contains("existing hook"));
        assert!(content.contains("souk ci run pre-commit"));
    }

    #[test]
    fn install_husky_skips_if_already_configured() {
        let tmp = TempDir::new().unwrap();
        let husky_dir = tmp.path().join(".husky");
        fs::create_dir(&husky_dir).unwrap();
        fs::write(
            husky_dir.join("pre-commit"),
            "souk ci run pre-commit\n",
        )
        .unwrap();

        let result = install_husky(tmp.path()).unwrap();
        assert!(result.contains("Already configured"));
    }

    #[test]
    fn hook_manager_name_returns_expected_values() {
        assert_eq!(HookManager::Native.name(), "native");
        assert_eq!(HookManager::Lefthook.name(), "lefthook");
        assert_eq!(HookManager::Husky.name(), "husky");
        assert_eq!(HookManager::Overcommit.name(), "overcommit");
        assert_eq!(HookManager::Hk.name(), "hk");
        assert_eq!(HookManager::SimpleGitHooks.name(), "simple-git-hooks");
    }

    #[test]
    fn hook_manager_display() {
        assert_eq!(format!("{}", HookManager::Lefthook), "lefthook");
        assert_eq!(format!("{}", HookManager::Native), "native");
    }
}
