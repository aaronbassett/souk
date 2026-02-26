use regex::Regex;
use std::sync::LazyLock;

static VERSION_CONSTRAINT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\*|[\^~]?[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?|(?:>=|<=|>|<|=)[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?)$",
    )
    .unwrap()
});

pub fn is_valid_version_constraint(s: &str) -> bool {
    VERSION_CONSTRAINT_RE.is_match(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_constraints() {
        let valid = [
            "*",
            "1.0.0",
            "^1.0.0",
            "~1.0.0",
            ">=1.0.0",
            "<=1.0.0",
            ">1.0.0",
            "<1.0.0",
            "^1.0.0-alpha.1",
            "1.2.3-beta",
        ];
        for v in valid {
            assert!(is_valid_version_constraint(v), "should be valid: {v}");
        }
    }

    #[test]
    fn invalid_constraints() {
        let invalid = ["", "latest", "1.0", "^1.0", ">>1.0.0", "1.0.0.0"];
        for v in invalid {
            assert!(!is_valid_version_constraint(v), "should be invalid: {v}");
        }
    }
}
