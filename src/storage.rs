use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Executor, Row, SqlitePool};
use tracing::info;

use crate::models::{
    DueEasterEggTaunt, EasterEggMessage, EasterEggSettings, Poll, PollNotification,
    RecurringSeries, Vote,
};

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn open(path: &str) -> Result<Self> {
        if let Some(parent) = Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .with_context(|| format!("failed to open SQLite database at {path}"))?;
        let store = Self { pool };
        store.migrate().await?;
        info!(database_path = %path, "opened SQLite database");
        Ok(store)
    }

    async fn migrate(&self) -> Result<()> {
        self.pool
            .execute(
                r#"
                CREATE TABLE IF NOT EXISTS choice_history (
                    normalized TEXT PRIMARY KEY,
                    choices_json TEXT NOT NULL,
                    display_text TEXT NOT NULL,
                    use_count INTEGER NOT NULL,
                    last_used_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS polls (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    description TEXT,
                    when_text TEXT,
                    location_text TEXT,
                    choices_json TEXT NOT NULL,
                    channel_id INTEGER NOT NULL,
                    message_id INTEGER,
                    recurring_id TEXT,
                    created_by INTEGER NOT NULL,
                    created_by_name TEXT,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS responses (
                    poll_id TEXT NOT NULL,
                    user_id INTEGER NOT NULL,
                    choice TEXT NOT NULL,
                    display_name TEXT,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY (poll_id, user_id),
                    FOREIGN KEY (poll_id) REFERENCES polls(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS recurring_series (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    description TEXT,
                    schedule TEXT NOT NULL,
                    when_text TEXT NOT NULL DEFAULT '',
                    location_text TEXT,
                    timezone TEXT NOT NULL,
                    choices_json TEXT NOT NULL,
                    notification_text TEXT,
                    notification_user_ids_json TEXT NOT NULL DEFAULT '[]',
                    notification_role_ids_json TEXT NOT NULL DEFAULT '[]',
                    channel_id INTEGER NOT NULL,
                    created_by INTEGER NOT NULL,
                    created_by_name TEXT,
                    next_post_at TEXT NOT NULL,
                    active INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS easter_egg_settings (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    enabled INTEGER NOT NULL,
                    target_user_id INTEGER NOT NULL,
                    channel_id INTEGER NOT NULL,
                    window_start_minute INTEGER NOT NULL,
                    window_end_minute INTEGER NOT NULL,
                    updated_by INTEGER NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS easter_egg_messages (
                    id TEXT PRIMARY KEY,
                    message TEXT NOT NULL,
                    created_by INTEGER NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS easter_egg_daily_runs (
                    run_date TEXT PRIMARY KEY,
                    roll INTEGER NOT NULL,
                    scheduled_at TEXT,
                    sent_at TEXT,
                    target_user_id INTEGER,
                    channel_id INTEGER,
                    message TEXT
                );
            "#,
            )
            .await?;
        self.ensure_column(
            "recurring_series",
            "notification_text",
            "ALTER TABLE recurring_series ADD COLUMN notification_text TEXT",
        )
        .await?;
        self.ensure_column(
            "recurring_series",
            "notification_user_ids_json",
            "ALTER TABLE recurring_series ADD COLUMN notification_user_ids_json TEXT NOT NULL DEFAULT '[]'",
        )
        .await?;
        self.ensure_column(
            "recurring_series",
            "notification_role_ids_json",
            "ALTER TABLE recurring_series ADD COLUMN notification_role_ids_json TEXT NOT NULL DEFAULT '[]'",
        )
        .await?;
        self.ensure_column(
            "responses",
            "display_name",
            "ALTER TABLE responses ADD COLUMN display_name TEXT",
        )
        .await?;
        self.ensure_column(
            "polls",
            "created_by_name",
            "ALTER TABLE polls ADD COLUMN created_by_name TEXT",
        )
        .await?;
        self.ensure_column(
            "polls",
            "location_text",
            "ALTER TABLE polls ADD COLUMN location_text TEXT",
        )
        .await?;
        self.ensure_column(
            "recurring_series",
            "location_text",
            "ALTER TABLE recurring_series ADD COLUMN location_text TEXT",
        )
        .await?;
        self.ensure_column(
            "recurring_series",
            "when_text",
            "ALTER TABLE recurring_series ADD COLUMN when_text TEXT NOT NULL DEFAULT ''",
        )
        .await?;
        self.ensure_column(
            "recurring_series",
            "created_by_name",
            "ALTER TABLE recurring_series ADD COLUMN created_by_name TEXT",
        )
        .await?;
        Ok(())
    }

    async fn ensure_column(&self, table: &str, column: &str, alter_sql: &str) -> Result<()> {
        let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(&self.pool)
            .await?;
        let exists = rows.iter().any(|row| {
            row.try_get::<String, _>("name")
                .is_ok_and(|name| name == column)
        });
        if !exists {
            self.pool.execute(alter_sql).await?;
        }
        Ok(())
    }

    pub async fn insert_poll(&self, poll: &Poll) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO polls
                (id, title, description, when_text, location_text, choices_json, channel_id, message_id,
                 recurring_id, created_by, created_by_name, created_at)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
        )
        .bind(&poll.id)
        .bind(&poll.title)
        .bind(&poll.description)
        .bind(&poll.when)
        .bind(&poll.location)
        .bind(serde_json::to_string(&poll.choices)?)
        .bind(to_i64(poll.channel_id)?)
        .bind(poll.message_id.map(to_i64).transpose()?)
        .bind(&poll.recurring_id)
        .bind(to_i64(poll.created_by)?)
        .bind(&poll.created_by_name)
        .bind(poll.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_poll_message(&self, poll_id: &str, message_id: u64) -> Result<()> {
        sqlx::query("UPDATE polls SET message_id = ?1 WHERE id = ?2")
            .bind(to_i64(message_id)?)
            .bind(poll_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_poll(&self, poll_id: &str) -> Result<Option<Poll>> {
        let row = sqlx::query(
            r#"
            SELECT id, title, description, when_text, location_text, choices_json, channel_id, message_id,
                   recurring_id, created_by, created_by_name, created_at
            FROM polls
            WHERE id = ?1
            "#,
        )
        .bind(poll_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_poll).transpose()
    }

    pub async fn set_response(
        &self,
        poll_id: &str,
        user_id: u64,
        display_name: &str,
        choice: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO responses (poll_id, user_id, choice, display_name, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(poll_id, user_id)
            DO UPDATE SET
                choice = excluded.choice,
                display_name = excluded.display_name,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(poll_id)
        .bind(to_i64(user_id)?)
        .bind(choice)
        .bind(display_name)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn poll_responses(&self, poll_id: &str) -> Result<Vec<Vote>> {
        let rows = sqlx::query(
            "SELECT user_id, display_name, choice FROM responses WHERE poll_id = ?1 ORDER BY updated_at",
        )
        .bind(poll_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_vote).collect()
    }

    pub async fn record_choice_history(&self, choices: &[String]) -> Result<()> {
        let display_text = choices.join(", ");
        let normalized = normalize_choices(choices);

        sqlx::query(
            r#"
            INSERT INTO choice_history
                (normalized, choices_json, display_text, use_count, last_used_at)
            VALUES
                (?1, ?2, ?3, 1, ?4)
            ON CONFLICT(normalized)
            DO UPDATE SET
                choices_json = excluded.choices_json,
                display_text = excluded.display_text,
                use_count = choice_history.use_count + 1,
                last_used_at = excluded.last_used_at
            "#,
        )
        .bind(normalized)
        .bind(serde_json::to_string(choices)?)
        .bind(display_text)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn recent_choice_sets(&self, partial: &str, limit: u32) -> Result<Vec<String>> {
        let pattern = format!("%{}%", partial.trim().to_lowercase());
        let rows = sqlx::query(
            r#"
            SELECT display_text
            FROM choice_history
            WHERE LOWER(display_text) LIKE ?1
            ORDER BY last_used_at DESC, use_count DESC
            LIMIT ?2
            "#,
        )
        .bind(pattern)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| row.try_get("display_text").map_err(Into::into))
            .collect()
    }

    pub async fn insert_series(&self, series: &RecurringSeries) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO recurring_series
                (id, title, description, schedule, when_text, timezone, choices_json,
                 location_text,
                 notification_text, notification_user_ids_json, notification_role_ids_json,
                 channel_id,
                 created_by, created_by_name, next_post_at, active, created_at)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 1, ?16)
            "#,
        )
        .bind(&series.id)
        .bind(&series.title)
        .bind(&series.description)
        .bind(&series.schedule)
        .bind(&series.when)
        .bind(series.timezone.name())
        .bind(serde_json::to_string(&series.choices)?)
        .bind(&series.location)
        .bind(
            series
                .notification
                .as_ref()
                .map(|notification| notification.content.as_str()),
        )
        .bind(serde_json::to_string(
            &series
                .notification
                .as_ref()
                .map(|notification| notification.user_ids.clone())
                .unwrap_or_default(),
        )?)
        .bind(serde_json::to_string(
            &series
                .notification
                .as_ref()
                .map(|notification| notification.role_ids.clone())
                .unwrap_or_default(),
        )?)
        .bind(to_i64(series.channel_id)?)
        .bind(to_i64(series.created_by)?)
        .bind(&series.created_by_name)
        .bind(series.next_post_at.to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_active_series(&self) -> Result<Vec<RecurringSeries>> {
        let rows = sqlx::query(
            r#"
            SELECT id, title, description, schedule, when_text, location_text, timezone, choices_json, channel_id,
                   notification_text, notification_user_ids_json, notification_role_ids_json,
                   created_by, created_by_name, next_post_at
            FROM recurring_series
            WHERE active = 1
            ORDER BY next_post_at
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_series).collect()
    }

    pub async fn due_series(&self, now: DateTime<Utc>) -> Result<Vec<RecurringSeries>> {
        let rows = sqlx::query(
            r#"
            SELECT id, title, description, schedule, when_text, location_text, timezone, choices_json, channel_id,
                   notification_text, notification_user_ids_json, notification_role_ids_json,
                   created_by, created_by_name, next_post_at
            FROM recurring_series
            WHERE active = 1 AND next_post_at <= ?1
            ORDER BY next_post_at
            "#,
        )
        .bind(now.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_series).collect()
    }

    pub async fn update_series_next_post(
        &self,
        id: &str,
        next_post_at: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query("UPDATE recurring_series SET next_post_at = ?1 WHERE id = ?2")
            .bind(next_post_at.to_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn deactivate_series(&self, id: &str) -> Result<bool> {
        let result =
            sqlx::query("UPDATE recurring_series SET active = 0 WHERE id = ?1 AND active = 1")
                .bind(id)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn upsert_easter_egg_settings(&self, settings: &EasterEggSettings) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO easter_egg_settings
                (id, enabled, target_user_id, channel_id, window_start_minute,
                 window_end_minute, updated_by, updated_at)
            VALUES
                (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id)
            DO UPDATE SET
                enabled = excluded.enabled,
                target_user_id = excluded.target_user_id,
                channel_id = excluded.channel_id,
                window_start_minute = excluded.window_start_minute,
                window_end_minute = excluded.window_end_minute,
                updated_by = excluded.updated_by,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(settings.enabled)
        .bind(to_i64(settings.target_user_id)?)
        .bind(to_i64(settings.channel_id)?)
        .bind(i64::from(settings.window_start_minute))
        .bind(i64::from(settings.window_end_minute))
        .bind(to_i64(settings.updated_by)?)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn easter_egg_settings(&self) -> Result<Option<EasterEggSettings>> {
        let row = sqlx::query(
            r#"
            SELECT enabled, target_user_id, channel_id, window_start_minute,
                   window_end_minute, updated_by
            FROM easter_egg_settings
            WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_easter_egg_settings).transpose()
    }

    pub async fn disable_easter_egg(&self) -> Result<bool> {
        let result = sqlx::query("UPDATE easter_egg_settings SET enabled = 0 WHERE id = 1")
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn add_easter_egg_message(
        &self,
        message: &EasterEggMessage,
        created_by: u64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO easter_egg_messages (id, message, created_by, created_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
        .bind(&message.id)
        .bind(&message.message)
        .bind(to_i64(created_by)?)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_easter_egg_messages(&self) -> Result<Vec<EasterEggMessage>> {
        let rows = sqlx::query("SELECT id, message FROM easter_egg_messages ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter().map(row_to_easter_egg_message).collect()
    }

    pub async fn easter_egg_run_exists(&self, run_date: &str) -> Result<bool> {
        let row = sqlx::query("SELECT 1 FROM easter_egg_daily_runs WHERE run_date = ?1")
            .bind(run_date)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.is_some())
    }

    pub async fn record_easter_egg_roll(
        &self,
        run_date: &str,
        roll: u8,
        scheduled_at: Option<DateTime<Utc>>,
        target_user_id: Option<u64>,
        channel_id: Option<u64>,
        message: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO easter_egg_daily_runs
                (run_date, roll, scheduled_at, target_user_id, channel_id, message)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(run_date)
        .bind(i64::from(roll))
        .bind(scheduled_at.map(|time| time.to_rfc3339()))
        .bind(target_user_id.map(to_i64).transpose()?)
        .bind(channel_id.map(to_i64).transpose()?)
        .bind(message)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn due_easter_egg_taunts(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<DueEasterEggTaunt>> {
        let rows = sqlx::query(
            r#"
            SELECT run_date, target_user_id, channel_id, message
            FROM easter_egg_daily_runs
            WHERE scheduled_at IS NOT NULL
              AND sent_at IS NULL
              AND scheduled_at <= ?1
            ORDER BY scheduled_at
            "#,
        )
        .bind(now.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_due_easter_egg_taunt).collect()
    }

    pub async fn mark_easter_egg_sent(&self, run_date: &str, sent_at: DateTime<Utc>) -> Result<()> {
        sqlx::query("UPDATE easter_egg_daily_runs SET sent_at = ?1 WHERE run_date = ?2")
            .bind(sent_at.to_rfc3339())
            .bind(run_date)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn row_to_poll(row: sqlx::sqlite::SqliteRow) -> Result<Poll> {
    let choices_json: String = row.try_get("choices_json")?;
    let created_at: String = row.try_get("created_at")?;

    Ok(Poll {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        when: row.try_get("when_text")?,
        location: row.try_get("location_text")?,
        choices: serde_json::from_str(&choices_json)?,
        channel_id: to_u64(row.try_get::<i64, _>("channel_id")?)?,
        message_id: row
            .try_get::<Option<i64>, _>("message_id")?
            .map(to_u64)
            .transpose()?,
        recurring_id: row.try_get("recurring_id")?,
        created_by: to_u64(row.try_get::<i64, _>("created_by")?)?,
        created_by_name: row.try_get("created_by_name")?,
        created_at: parse_utc(&created_at)?,
    })
}

fn row_to_vote(row: sqlx::sqlite::SqliteRow) -> Result<Vote> {
    Ok(Vote {
        user_id: to_u64(row.try_get::<i64, _>("user_id")?)?,
        display_name: row.try_get("display_name")?,
        choice: row.try_get("choice")?,
    })
}

fn row_to_series(row: sqlx::sqlite::SqliteRow) -> Result<RecurringSeries> {
    let timezone: String = row.try_get("timezone")?;
    let choices_json: String = row.try_get("choices_json")?;
    let next_post_at: String = row.try_get("next_post_at")?;

    Ok(RecurringSeries {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        schedule: row.try_get("schedule")?,
        when: row.try_get("when_text")?,
        location: row.try_get("location_text")?,
        timezone: timezone.parse().unwrap_or(chrono_tz::UTC),
        choices: serde_json::from_str(&choices_json)?,
        notification: row_to_notification(&row)?,
        channel_id: to_u64(row.try_get::<i64, _>("channel_id")?)?,
        created_by: to_u64(row.try_get::<i64, _>("created_by")?)?,
        created_by_name: row.try_get("created_by_name")?,
        next_post_at: parse_utc(&next_post_at)?,
    })
}

fn row_to_notification(row: &sqlx::sqlite::SqliteRow) -> Result<Option<PollNotification>> {
    let Some(content) = row.try_get::<Option<String>, _>("notification_text")? else {
        return Ok(None);
    };
    let user_ids_json: String = row.try_get("notification_user_ids_json")?;
    let role_ids_json: String = row.try_get("notification_role_ids_json")?;

    Ok(Some(PollNotification {
        content,
        user_ids: serde_json::from_str(&user_ids_json)?,
        role_ids: serde_json::from_str(&role_ids_json)?,
    }))
}

fn row_to_easter_egg_settings(row: sqlx::sqlite::SqliteRow) -> Result<EasterEggSettings> {
    Ok(EasterEggSettings {
        enabled: row.try_get::<bool, _>("enabled")?,
        target_user_id: to_u64(row.try_get::<i64, _>("target_user_id")?)?,
        channel_id: to_u64(row.try_get::<i64, _>("channel_id")?)?,
        window_start_minute: to_u16(row.try_get::<i64, _>("window_start_minute")?)?,
        window_end_minute: to_u16(row.try_get::<i64, _>("window_end_minute")?)?,
        updated_by: to_u64(row.try_get::<i64, _>("updated_by")?)?,
    })
}

fn row_to_easter_egg_message(row: sqlx::sqlite::SqliteRow) -> Result<EasterEggMessage> {
    Ok(EasterEggMessage {
        id: row.try_get("id")?,
        message: row.try_get("message")?,
    })
}

fn row_to_due_easter_egg_taunt(row: sqlx::sqlite::SqliteRow) -> Result<DueEasterEggTaunt> {
    Ok(DueEasterEggTaunt {
        run_date: row.try_get("run_date")?,
        target_user_id: to_u64(row.try_get::<i64, _>("target_user_id")?)?,
        channel_id: to_u64(row.try_get::<i64, _>("channel_id")?)?,
        message: row.try_get("message")?,
    })
}

fn parse_utc(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn to_i64(value: u64) -> Result<i64> {
    i64::try_from(value).context("Discord ID did not fit in SQLite integer")
}

fn to_u64(value: i64) -> Result<u64> {
    u64::try_from(value).context("stored Discord ID was negative")
}

fn to_u16(value: i64) -> Result<u16> {
    u16::try_from(value).context("stored minute value did not fit in u16")
}

fn normalize_choices(choices: &[String]) -> String {
    choices
        .iter()
        .map(|choice| choice.trim().to_lowercase())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_and_suggests_recent_choice_sets() {
        let path =
            std::env::temp_dir().join(format!("urinal-fish-test-{}.db", uuid::Uuid::new_v4()));
        let path = path.to_string_lossy().into_owned();
        let store = Store::open(&path).await.unwrap();

        store
            .record_choice_history(&["yes".into(), "no".into(), "maybe".into()])
            .await
            .unwrap();
        store
            .record_choice_history(&["Pizza".into(), "Sushi".into(), "No".into()])
            .await
            .unwrap();

        let suggestions = store.recent_choice_sets("piz", 10).await.unwrap();

        assert_eq!(suggestions, ["Pizza, Sushi, No"]);

        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn normalizes_choices_case_insensitively() {
        let normalized = normalize_choices(&[" Yes ".into(), "NO".into()]);

        assert_eq!(normalized, "yes\nno");
    }
}
