use std::collections::HashSet;

use anyhow::{Result, anyhow};

use crate::validation;

pub const DEFAULT_CHOICES: [&str; 3] = ["yes", "no", "maybe"];

pub fn default_choices() -> Vec<String> {
    DEFAULT_CHOICES
        .iter()
        .map(|choice| choice.to_string())
        .collect()
}

pub fn parse_choices(value: &str) -> Result<Vec<String>> {
    let choices = value
        .split(',')
        .map(str::trim)
        .filter(|choice| !choice.is_empty())
        .map(|choice| validation::clean_text(choice.to_string(), "choice", 60))
        .collect::<Result<Vec<_>>>()?;

    if choices.len() < 2 {
        return Err(anyhow!("provide at least two comma-separated choices"));
    }
    if choices.len() > 25 {
        return Err(anyhow!("Discord buttons support at most 25 choices"));
    }

    let mut seen = HashSet::new();
    for choice in &choices {
        if !seen.insert(choice.to_lowercase()) {
            return Err(anyhow!("choice labels must be unique"));
        }
    }

    Ok(choices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comma_separated_choices() {
        let choices = parse_choices("yes, no, maybe").unwrap();

        assert_eq!(choices, ["yes", "no", "maybe"]);
    }

    #[test]
    fn rejects_duplicate_choices_case_insensitively() {
        let error = parse_choices("yes, no, YES").unwrap_err().to_string();

        assert!(error.contains("unique"));
    }

    #[test]
    fn rejects_less_than_two_choices() {
        let error = parse_choices("yes").unwrap_err().to_string();

        assert!(error.contains("at least two"));
    }
}
