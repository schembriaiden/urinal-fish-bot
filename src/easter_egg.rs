use anyhow::{Context, Result};
use chrono::{DateTime, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use uuid::Uuid;

const DAILY_ROLL_HOUR: u32 = 4;
const WINNING_ROLL: u8 = 11;
const ROLL_SIDES: u8 = 20;

pub fn roll_cutoff_date(now: DateTime<Utc>, timezone: Tz) -> Option<NaiveDate> {
    let local_now = now.with_timezone(&timezone);
    let cutoff = NaiveTime::from_hms_opt(DAILY_ROLL_HOUR, 0, 0).expect("valid cutoff time");

    (local_now.time() >= cutoff).then_some(local_now.date_naive())
}

pub fn roll_d20() -> u8 {
    random_range_inclusive(1, u64::from(ROLL_SIDES)) as u8
}

pub fn is_winning_roll(roll: u8) -> bool {
    roll == WINNING_ROLL
}

pub fn choose_message(messages: &[String]) -> Option<String> {
    if messages.is_empty() {
        return None;
    }

    let index = random_range_inclusive(0, messages.len() as u64 - 1) as usize;
    Some(messages[index].clone())
}

pub fn random_scheduled_at(
    date: NaiveDate,
    start_minute: u16,
    end_minute: u16,
    timezone: Tz,
) -> Result<DateTime<Utc>> {
    let minute = random_range_inclusive(start_minute as u64, end_minute as u64) as u16;
    scheduled_at(date, minute, timezone)
}

pub fn scheduled_at(date: NaiveDate, minute_of_day: u16, timezone: Tz) -> Result<DateTime<Utc>> {
    let hour = u32::from(minute_of_day / 60);
    let minute = u32::from(minute_of_day % 60);
    let time = NaiveTime::from_hms_opt(hour, minute, 0)
        .with_context(|| format!("invalid minute of day {minute_of_day}"))?;
    let naive = date.and_time(time);

    match timezone.from_local_datetime(&naive) {
        LocalResult::Single(time) => Ok(time.with_timezone(&Utc)),
        LocalResult::Ambiguous(earlier, _) => Ok(earlier.with_timezone(&Utc)),
        LocalResult::None => anyhow::bail!("scheduled local time does not exist in {timezone}"),
    }
}

pub fn parse_time(value: &str, field: &str) -> Result<u16> {
    let (hour, minute) = value
        .trim()
        .split_once(':')
        .with_context(|| format!("{field} must use HH:MM format"))?;
    let hour = hour
        .parse::<u16>()
        .with_context(|| format!("{field} has an invalid hour"))?;
    let minute = minute
        .parse::<u16>()
        .with_context(|| format!("{field} has an invalid minute"))?;

    anyhow::ensure!(hour < 24, "{field} hour must be between 00 and 23");
    anyhow::ensure!(minute < 60, "{field} minute must be between 00 and 59");

    Ok(hour * 60 + minute)
}

pub fn validate_window(start_minute: u16, end_minute: u16) -> Result<()> {
    let cutoff_minute = (DAILY_ROLL_HOUR * 60) as u16;
    anyhow::ensure!(
        start_minute >= cutoff_minute,
        "start time must be 04:00 or later because the daily roll happens at 04:00"
    );
    anyhow::ensure!(
        start_minute <= end_minute,
        "start time must be before or equal to end time"
    );
    Ok(())
}

pub fn format_time(minute_of_day: u16) -> String {
    format!("{:02}:{:02}", minute_of_day / 60, minute_of_day % 60)
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
    fn parses_valid_time() {
        let minute = parse_time("13:45", "time").unwrap();

        assert_eq!(minute, 825);
    }

    #[test]
    fn rejects_invalid_time() {
        let error = parse_time("25:00", "time").unwrap_err().to_string();

        assert!(error.contains("between 00 and 23"));
    }

    #[test]
    fn validates_window_after_roll_cutoff() {
        let error = validate_window(180, 240).unwrap_err().to_string();

        assert!(error.contains("04:00"));
    }

    #[test]
    fn detects_roll_cutoff_date() {
        let timezone = chrono_tz::Europe::Malta;
        let before = "2026-06-17T01:59:00Z".parse::<DateTime<Utc>>().unwrap();
        let after = "2026-06-17T02:00:00Z".parse::<DateTime<Utc>>().unwrap();

        assert_eq!(roll_cutoff_date(before, timezone), None);
        assert_eq!(
            roll_cutoff_date(after, timezone).unwrap().to_string(),
            "2026-06-17"
        );
    }

    #[test]
    fn winning_roll_is_eleven() {
        assert!(is_winning_roll(11));
        assert!(!is_winning_roll(12));
    }
}
