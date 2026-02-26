//! Semantic version bumping and unique name generation.
//!
//! Provides helpers for incrementing semver version strings and generating
//! unique plugin names when name conflicts arise during `souk add`.
//!
//! Version strings are parsed with the [`semver`] crate to ensure correctness.
//! Pre-release and build metadata are stripped on bump, following standard
//! semver increment semantics.

use std::collections::HashSet;

use crate::SoukError;

/// Bumps the major component of a semver version string.
///
/// The minor and patch components are reset to zero and any pre-release /
/// build metadata is dropped.
///
/// # Examples
///
/// ```
/// # use souk_core::version::bump_major;
/// assert_eq!(bump_major("1.2.3").unwrap(), "2.0.0");
/// assert_eq!(bump_major("0.9.1").unwrap(), "1.0.0");
/// assert_eq!(bump_major("1.2.3-beta.1").unwrap(), "2.0.0");
/// ```
///
/// # Errors
///
/// Returns [`SoukError::Semver`] if `version` is not a valid semver string.
pub fn bump_major(version: &str) -> Result<String, SoukError> {
    let v = semver::Version::parse(version)?;
    let bumped = semver::Version::new(v.major + 1, 0, 0);
    Ok(bumped.to_string())
}

/// Bumps the minor component of a semver version string.
///
/// The patch component is reset to zero and any pre-release / build metadata
/// is dropped.
///
/// # Examples
///
/// ```
/// # use souk_core::version::bump_minor;
/// assert_eq!(bump_minor("1.2.3").unwrap(), "1.3.0");
/// assert_eq!(bump_minor("0.1.0").unwrap(), "0.2.0");
/// assert_eq!(bump_minor("2.0.0-rc.1").unwrap(), "2.1.0");
/// ```
///
/// # Errors
///
/// Returns [`SoukError::Semver`] if `version` is not a valid semver string.
pub fn bump_minor(version: &str) -> Result<String, SoukError> {
    let v = semver::Version::parse(version)?;
    let bumped = semver::Version::new(v.major, v.minor + 1, 0);
    Ok(bumped.to_string())
}

/// Bumps the patch component of a semver version string.
///
/// Any pre-release / build metadata is dropped.
///
/// # Examples
///
/// ```
/// # use souk_core::version::bump_patch;
/// assert_eq!(bump_patch("1.2.3").unwrap(), "1.2.4");
/// assert_eq!(bump_patch("0.0.0").unwrap(), "0.0.1");
/// assert_eq!(bump_patch("3.1.4-alpha").unwrap(), "3.1.5");
/// ```
///
/// # Errors
///
/// Returns [`SoukError::Semver`] if `version` is not a valid semver string.
pub fn bump_patch(version: &str) -> Result<String, SoukError> {
    let v = semver::Version::parse(version)?;
    let bumped = semver::Version::new(v.major, v.minor, v.patch + 1);
    Ok(bumped.to_string())
}

/// Generates a unique name by appending a numeric suffix if `base` already
/// exists in `existing`.
///
/// If `base` is not in `existing`, it is returned unchanged. Otherwise, the
/// function tries `base-2`, `base-3`, etc. until a name not in `existing` is
/// found.
///
/// This mirrors the shell-based `generate_unique_plugin_name` from the
/// reference scripts in `temp-reference-scripts/lib/atomic.sh`.
///
/// # Examples
///
/// ```
/// # use std::collections::HashSet;
/// # use souk_core::version::generate_unique_name;
/// let existing: HashSet<String> = ["foo".into(), "foo-2".into()].into();
/// assert_eq!(generate_unique_name("foo", &existing), "foo-3");
/// assert_eq!(generate_unique_name("bar", &existing), "bar");
/// ```
pub fn generate_unique_name(base: &str, existing: &HashSet<String>) -> String {
    if !existing.contains(base) {
        return base.to_string();
    }

    let mut counter = 2u64;
    loop {
        let candidate = format!("{base}-{counter}");
        if !existing.contains(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // bump_major
    // -----------------------------------------------------------------------

    #[test]
    fn bump_major_standard() {
        assert_eq!(bump_major("1.2.3").unwrap(), "2.0.0");
    }

    #[test]
    fn bump_major_from_zero() {
        assert_eq!(bump_major("0.1.0").unwrap(), "1.0.0");
    }

    #[test]
    fn bump_major_strips_prerelease() {
        assert_eq!(bump_major("1.2.3-beta.1").unwrap(), "2.0.0");
    }

    #[test]
    fn bump_major_strips_build_metadata() {
        assert_eq!(bump_major("1.2.3+build.42").unwrap(), "2.0.0");
    }

    #[test]
    fn bump_major_resets_minor_and_patch() {
        assert_eq!(bump_major("3.9.27").unwrap(), "4.0.0");
    }

    #[test]
    fn bump_major_invalid_version() {
        assert!(bump_major("not-a-version").is_err());
    }

    #[test]
    fn bump_major_incomplete_version() {
        // semver crate requires all three components
        assert!(bump_major("1.2").is_err());
    }

    // -----------------------------------------------------------------------
    // bump_minor
    // -----------------------------------------------------------------------

    #[test]
    fn bump_minor_standard() {
        assert_eq!(bump_minor("1.2.3").unwrap(), "1.3.0");
    }

    #[test]
    fn bump_minor_from_zero() {
        assert_eq!(bump_minor("0.0.0").unwrap(), "0.1.0");
    }

    #[test]
    fn bump_minor_strips_prerelease() {
        assert_eq!(bump_minor("2.0.0-rc.1").unwrap(), "2.1.0");
    }

    #[test]
    fn bump_minor_resets_patch() {
        assert_eq!(bump_minor("1.5.99").unwrap(), "1.6.0");
    }

    #[test]
    fn bump_minor_zero_x_version() {
        assert_eq!(bump_minor("0.9.1").unwrap(), "0.10.0");
    }

    #[test]
    fn bump_minor_invalid_version() {
        assert!(bump_minor("abc").is_err());
    }

    // -----------------------------------------------------------------------
    // bump_patch
    // -----------------------------------------------------------------------

    #[test]
    fn bump_patch_standard() {
        assert_eq!(bump_patch("1.2.3").unwrap(), "1.2.4");
    }

    #[test]
    fn bump_patch_from_zero() {
        assert_eq!(bump_patch("0.0.0").unwrap(), "0.0.1");
    }

    #[test]
    fn bump_patch_strips_prerelease() {
        assert_eq!(bump_patch("3.1.4-alpha").unwrap(), "3.1.5");
    }

    #[test]
    fn bump_patch_large_numbers() {
        assert_eq!(bump_patch("999.999.999").unwrap(), "999.999.1000");
    }

    #[test]
    fn bump_patch_invalid_version() {
        assert!(bump_patch("").is_err());
    }

    #[test]
    fn bump_patch_with_build_and_prerelease() {
        assert_eq!(bump_patch("1.0.0-alpha+build.1").unwrap(), "1.0.1");
    }

    // -----------------------------------------------------------------------
    // generate_unique_name
    // -----------------------------------------------------------------------

    #[test]
    fn unique_name_no_conflict() {
        let existing: HashSet<String> = HashSet::new();
        assert_eq!(generate_unique_name("my-plugin", &existing), "my-plugin");
    }

    #[test]
    fn unique_name_base_conflict() {
        let existing: HashSet<String> = ["my-plugin".into()].into();
        assert_eq!(
            generate_unique_name("my-plugin", &existing),
            "my-plugin-2"
        );
    }

    #[test]
    fn unique_name_multiple_conflicts() {
        let existing: HashSet<String> =
            ["foo".into(), "foo-2".into(), "foo-3".into()].into();
        assert_eq!(generate_unique_name("foo", &existing), "foo-4");
    }

    #[test]
    fn unique_name_gap_in_numbers() {
        // If foo and foo-3 exist but foo-2 does not, it should pick foo-2.
        let existing: HashSet<String> = ["foo".into(), "foo-3".into()].into();
        assert_eq!(generate_unique_name("foo", &existing), "foo-2");
    }

    #[test]
    fn unique_name_with_existing_suffix() {
        // Even if the base name itself has a number suffix, it works correctly.
        let existing: HashSet<String> = ["plugin-2".into()].into();
        assert_eq!(generate_unique_name("plugin-2", &existing), "plugin-2-2");
    }

    #[test]
    fn unique_name_empty_base() {
        let existing: HashSet<String> = ["".into()].into();
        assert_eq!(generate_unique_name("", &existing), "-2");
    }
}
