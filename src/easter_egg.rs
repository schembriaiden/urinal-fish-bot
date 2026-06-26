use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use uuid::Uuid;

pub fn trigger_date(now: DateTime<Utc>, timezone: Tz) -> NaiveDate {
    now.with_timezone(&timezone).date_naive()
}

pub fn choose_message(messages: &[String]) -> Option<String> {
    if messages.is_empty() {
        return None;
    }

    let index = random_range_inclusive(0, messages.len() as u64 - 1) as usize;
    Some(messages[index].clone())
}

pub fn format_taunt_message(target_user_id: u64, message: &str) -> String {
    format!("<@{target_user_id}> {message}")
}

fn random_range_inclusive(min: u64, max: u64) -> u64 {
    debug_assert!(min <= max);
    let random = u128::from_be_bytes(*Uuid::new_v4().as_bytes());
    let range = u128::from(max - min + 1);
    min + (random % range) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_date_uses_configured_timezone() {
        let timezone = chrono_tz::Europe::Berlin;
        let now = "2026-06-17T22:30:00Z".parse::<DateTime<Utc>>().unwrap();

        assert_eq!(trigger_date(now, timezone).to_string(), "2026-06-18");
    }

    #[test]
    fn formats_taunt_message_with_target_mention() {
        let message = format_taunt_message(123, "Bring a permission slip.");

        assert_eq!(message, "<@123> Bring a permission slip.");
    }
}
