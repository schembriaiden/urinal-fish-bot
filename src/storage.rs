use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Executor, Row, SqlitePool};

use crate::models::{ChoiceTemplate, Poll, RecurringSeries, Vote};

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn open(path: &str) -> Result<Self> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
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
        Ok(store)
    }

    async fn migrate(&self) -> Result<()> {
        self.pool
            .execute(
                r#"
                CREATE TABLE IF NOT EXISTS choice_templates (
                    name TEXT PRIMARY KEY,
                    choices_json TEXT NOT NULL,
                    created_by INTEGER NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS polls (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    description TEXT,
                    when_text TEXT,
                    choices_json TEXT NOT NULL,
                    channel_id INTEGER NOT NULL,
                    message_id INTEGER,
                    recurring_id TEXT,
                    created_by INTEGER NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS responses (
                    poll_id TEXT NOT NULL,
                    user_id INTEGER NOT NULL,
                    choice TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY (poll_id, user_id),
                    FOREIGN KEY (poll_id) REFERENCES polls(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS recurring_series (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    description TEXT,
                    schedule TEXT NOT NULL,
                    timezone TEXT NOT NULL,
                    choices_json TEXT NOT NULL,
                    channel_id INTEGER NOT NULL,
                    created_by INTEGER NOT NULL,
                    next_post_at TEXT NOT NULL,
                    active INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL
                );
                "#,
            )
            .await?;
        Ok(())
    }

    pub async fn insert_poll(&self, poll: &Poll) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO polls
                (id, title, description, when_text, choices_json, channel_id, message_id, recurring_id, created_by, created_at)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(&poll.id)
        .bind(&poll.title)
        .bind(&poll.description)
        .bind(&poll.when)
        .bind(serde_json::to_string(&poll.choices)?)
        .bind(to_i64(poll.channel_id)?)
        .bind(poll.message_id.map(to_i64).transpose()?)
        .bind(&poll.recurring_id)
        .bind(to_i64(poll.created_by)?)
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
            SELECT id, title, description, when_text, choices_json, channel_id, message_id,
                   recurring_id, created_by, created_at
            FROM polls
            WHERE id = ?1
            "#,
        )
        .bind(poll_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_poll).transpose()
    }

    pub async fn set_response(&self, poll_id: &str, user_id: u64, choice: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO responses (poll_id, user_id, choice, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(poll_id, user_id)
            DO UPDATE SET choice = excluded.choice, updated_at = excluded.updated_at
            "#,
        )
        .bind(poll_id)
        .bind(to_i64(user_id)?)
        .bind(choice)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn poll_responses(&self, poll_id: &str) -> Result<Vec<Vote>> {
        let rows = sqlx::query(
            "SELECT user_id, choice FROM responses WHERE poll_id = ?1 ORDER BY updated_at",
        )
        .bind(poll_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_vote).collect()
    }

    pub async fn save_template(
        &self,
        name: &str,
        choices: &[String],
        created_by: u64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO choice_templates (name, choices_json, created_by, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(name)
            DO UPDATE SET choices_json = excluded.choices_json
            "#,
        )
        .bind(name)
        .bind(serde_json::to_string(choices)?)
        .bind(to_i64(created_by)?)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_template(&self, name: &str) -> Result<Option<ChoiceTemplate>> {
        let row = sqlx::query("SELECT name, choices_json FROM choice_templates WHERE name = ?1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        row.map(row_to_template).transpose()
    }

    pub async fn list_templates(&self) -> Result<Vec<ChoiceTemplate>> {
        let rows = sqlx::query("SELECT name, choices_json FROM choice_templates ORDER BY name")
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter().map(row_to_template).collect()
    }

    pub async fn delete_template(&self, name: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM choice_templates WHERE name = ?1")
            .bind(name)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn insert_series(&self, series: &RecurringSeries) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO recurring_series
                (id, title, description, schedule, timezone, choices_json, channel_id,
                 created_by, next_post_at, active, created_at)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10)
            "#,
        )
        .bind(&series.id)
        .bind(&series.title)
        .bind(&series.description)
        .bind(&series.schedule)
        .bind(series.timezone.name())
        .bind(serde_json::to_string(&series.choices)?)
        .bind(to_i64(series.channel_id)?)
        .bind(to_i64(series.created_by)?)
        .bind(series.next_post_at.to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_active_series(&self) -> Result<Vec<RecurringSeries>> {
        let rows = sqlx::query(
            r#"
            SELECT id, title, description, schedule, timezone, choices_json, channel_id,
                   created_by, next_post_at
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
            SELECT id, title, description, schedule, timezone, choices_json, channel_id,
                   created_by, next_post_at
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
}

fn row_to_poll(row: sqlx::sqlite::SqliteRow) -> Result<Poll> {
    let choices_json: String = row.try_get("choices_json")?;
    let created_at: String = row.try_get("created_at")?;

    Ok(Poll {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        when: row.try_get("when_text")?,
        choices: serde_json::from_str(&choices_json)?,
        channel_id: to_u64(row.try_get::<i64, _>("channel_id")?)?,
        message_id: row
            .try_get::<Option<i64>, _>("message_id")?
            .map(to_u64)
            .transpose()?,
        recurring_id: row.try_get("recurring_id")?,
        created_by: to_u64(row.try_get::<i64, _>("created_by")?)?,
        created_at: parse_utc(&created_at)?,
    })
}

fn row_to_vote(row: sqlx::sqlite::SqliteRow) -> Result<Vote> {
    Ok(Vote {
        user_id: to_u64(row.try_get::<i64, _>("user_id")?)?,
        choice: row.try_get("choice")?,
    })
}

fn row_to_template(row: sqlx::sqlite::SqliteRow) -> Result<ChoiceTemplate> {
    let choices_json: String = row.try_get("choices_json")?;

    Ok(ChoiceTemplate {
        name: row.try_get("name")?,
        choices: serde_json::from_str(&choices_json)?,
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
        timezone: timezone.parse().unwrap_or(chrono_tz::UTC),
        choices: serde_json::from_str(&choices_json)?,
        channel_id: to_u64(row.try_get::<i64, _>("channel_id")?)?,
        created_by: to_u64(row.try_get::<i64, _>("created_by")?)?,
        next_post_at: parse_utc(&next_post_at)?,
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
