use anyhow::{Result, anyhow};

const MAX_TITLE_LEN: usize = 100;
const MAX_DESCRIPTION_LEN: usize = 800;
const MAX_WHEN_LEN: usize = 120;

pub fn poll_title(value: String) -> Result<String> {
    clean_text(value, "title", MAX_TITLE_LEN)
}

pub fn optional_description(value: Option<String>) -> Result<Option<String>> {
    value
        .map(|value| clean_text(value, "description", MAX_DESCRIPTION_LEN))
        .transpose()
}

pub fn optional_when(value: Option<String>) -> Result<Option<String>> {
    value
        .map(|value| clean_text(value, "when", MAX_WHEN_LEN))
        .transpose()
}

pub fn clean_text(value: String, field: &str, max_len: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(anyhow!("{field} cannot be empty"));
    }
    if value.chars().count() > max_len {
        return Err(anyhow!("{field} must be {max_len} characters or less"));
    }
    if value
        .chars()
        .any(|character| character.is_control() && character != '\n' && character != '\t')
    {
        return Err(anyhow!("{field} contains unsupported control characters"));
    }

    Ok(neutralize_mentions(value))
}

fn neutralize_mentions(value: &str) -> String {
    value.replace('@', "@\u{200B}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutralizes_mentions() {
        let value = poll_title("@everyone Friday".to_string()).unwrap();

        assert_ne!(value, "@everyone Friday");
        assert!(value.contains("@\u{200B}everyone"));
    }

    #[test]
    fn rejects_overlong_title() {
        let error = poll_title("x".repeat(101)).unwrap_err().to_string();

        assert!(error.contains("100"));
    }
}
