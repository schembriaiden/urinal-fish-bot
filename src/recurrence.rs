use anyhow::{Context, Result, anyhow};
use chrono::{
    DateTime, Datelike, Days, LocalResult, NaiveDate, NaiveTime, TimeZone, Timelike, Utc, Weekday,
};
use chrono_tz::Tz;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recurrence {
    Daily { time: NaiveTime },
    Weekly { weekday: Weekday, time: NaiveTime },
    Monthly { day: u32, time: NaiveTime },
}

impl Recurrence {
    pub fn parse(raw: &str) -> Result<Self> {
        let cleaned = raw.to_lowercase().replace(',', " ");
        let mut parts = cleaned.split_whitespace().collect::<Vec<_>>();
        if parts.first() == Some(&"every") {
            parts.remove(0);
        }

        match parts.as_slice() {
            ["daily", time] | ["everyday", time] => Ok(Self::Daily {
                time: parse_time(time)?,
            }),
            ["weekly", weekday, time] => Ok(Self::Weekly {
                weekday: parse_weekday(weekday)?,
                time: parse_time(time)?,
            }),
            [weekday, time] if parse_weekday(weekday).is_ok() => Ok(Self::Weekly {
                weekday: parse_weekday(weekday)?,
                time: parse_time(time)?,
            }),
            ["monthly", day, time] => {
                let day = day.parse::<u32>().context("monthly day must be a number")?;
                if !(1..=31).contains(&day) {
                    return Err(anyhow!("monthly day must be between 1 and 31"));
                }
                Ok(Self::Monthly {
                    day,
                    time: parse_time(time)?,
                })
            }
            _ => Err(anyhow!(
                "use schedules like `daily 19:00`, `weekly fri 20:00`, or `monthly 15 19:30`"
            )),
        }
    }

    pub fn next_after(&self, timezone: Tz, after: DateTime<Utc>) -> Result<DateTime<Utc>> {
        let after_local = after.with_timezone(&timezone);
        let candidate = match self {
            Self::Daily { time } => {
                let mut date = after_local.date_naive();
                let mut local = local_datetime(timezone, date, *time)?;
                while local <= after {
                    date = date
                        .checked_add_days(Days::new(1))
                        .ok_or_else(|| anyhow!("date overflow"))?;
                    local = local_datetime(timezone, date, *time)?;
                }
                local
            }
            Self::Weekly { weekday, time } => {
                let mut date = after_local.date_naive();
                let current = date.weekday().num_days_from_monday() as i64;
                let target = weekday.num_days_from_monday() as i64;
                let days_until = (target - current).rem_euclid(7);
                date = date
                    .checked_add_days(Days::new(days_until as u64))
                    .ok_or_else(|| anyhow!("date overflow"))?;
                let mut local = local_datetime(timezone, date, *time)?;
                if local <= after {
                    date = date
                        .checked_add_days(Days::new(7))
                        .ok_or_else(|| anyhow!("date overflow"))?;
                    local = local_datetime(timezone, date, *time)?;
                }
                local
            }
            Self::Monthly { day, time } => {
                let mut year = after_local.year();
                let mut month = after_local.month();
                loop {
                    let date = clamped_month_date(year, month, *day)?;
                    let local = local_datetime(timezone, date, *time)?;
                    if local > after {
                        break local;
                    }
                    if month == 12 {
                        year += 1;
                        month = 1;
                    } else {
                        month += 1;
                    }
                }
            }
        };

        Ok(candidate)
    }
}

pub fn next_occurrence(
    schedule: &str,
    timezone: Tz,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>> {
    Recurrence::parse(schedule)?.next_after(timezone, after)
}

fn parse_time(raw: &str) -> Result<NaiveTime> {
    NaiveTime::parse_from_str(raw, "%H:%M").context("time must be HH:MM, for example 19:30")
}

fn parse_weekday(raw: &str) -> Result<Weekday> {
    match raw {
        "mon" | "monday" => Ok(Weekday::Mon),
        "tue" | "tues" | "tuesday" => Ok(Weekday::Tue),
        "wed" | "wednesday" => Ok(Weekday::Wed),
        "thu" | "thur" | "thurs" | "thursday" => Ok(Weekday::Thu),
        "fri" | "friday" => Ok(Weekday::Fri),
        "sat" | "saturday" => Ok(Weekday::Sat),
        "sun" | "sunday" => Ok(Weekday::Sun),
        _ => Err(anyhow!("unknown weekday")),
    }
}

fn local_datetime(timezone: Tz, date: NaiveDate, time: NaiveTime) -> Result<DateTime<Utc>> {
    match timezone.from_local_datetime(&date.and_time(time)) {
        LocalResult::Single(value) => Ok(value.with_timezone(&Utc)),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest.with_timezone(&Utc)),
        LocalResult::None => {
            warn!("local time {date} {time} does not exist in {timezone}; trying one hour later");
            let adjusted = date
                .and_hms_opt(time.hour(), time.minute(), 0)
                .ok_or_else(|| anyhow!("invalid local time"))?
                + chrono::Duration::hours(1);
            match timezone.from_local_datetime(&adjusted) {
                LocalResult::Single(value) | LocalResult::Ambiguous(value, _) => {
                    Ok(value.with_timezone(&Utc))
                }
                LocalResult::None => Err(anyhow!("local time does not exist in timezone")),
            }
        }
    }
}

fn clamped_month_date(year: i32, month: u32, wanted_day: u32) -> Result<NaiveDate> {
    let mut day = wanted_day;
    while day > 0 {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            return Ok(date);
        }
        day -= 1;
    }
    Err(anyhow!("invalid month"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn parses_weekly_schedule() {
        let parsed = Recurrence::parse("weekly fri 20:00").unwrap();

        assert_eq!(
            parsed,
            Recurrence::Weekly {
                weekday: Weekday::Fri,
                time: NaiveTime::from_hms_opt(20, 0, 0).unwrap(),
            }
        );
    }

    #[test]
    fn finds_next_daily_occurrence() {
        let after = Utc.with_ymd_and_hms(2026, 6, 17, 17, 0, 0).unwrap();
        let next = next_occurrence("daily 20:00", chrono_tz::Europe::Berlin, after).unwrap();

        assert_eq!(next, Utc.with_ymd_and_hms(2026, 6, 17, 18, 0, 0).unwrap());
    }

    #[test]
    fn rolls_weekly_occurrence_to_following_week() {
        let after = Utc.with_ymd_and_hms(2026, 6, 19, 19, 0, 0).unwrap();
        let next = next_occurrence("weekly fri 20:00", chrono_tz::Europe::Berlin, after).unwrap();

        assert_eq!(next, Utc.with_ymd_and_hms(2026, 6, 26, 18, 0, 0).unwrap());
    }

    #[test]
    fn clamps_monthly_day_to_end_of_month() {
        let after = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        let next = next_occurrence("monthly 31 20:00", chrono_tz::Europe::Berlin, after).unwrap();

        assert_eq!(next, Utc.with_ymd_and_hms(2026, 2, 28, 19, 0, 0).unwrap());
    }
}
