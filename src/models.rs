use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poll {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub when: Option<String>,
    pub choices: Vec<String>,
    pub channel_id: u64,
    pub message_id: Option<u64>,
    pub recurring_id: Option<String>,
    pub created_by: u64,
    pub created_at: DateTime<Utc>,
}

impl Poll {
    pub fn new(
        title: String,
        description: Option<String>,
        when: Option<String>,
        choices: Vec<String>,
        channel_id: u64,
        recurring_id: Option<String>,
        created_by: u64,
    ) -> Self {
        Self {
            id: short_id(),
            title,
            description,
            when,
            choices,
            channel_id,
            message_id: None,
            recurring_id,
            created_by,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChoiceTemplate {
    pub name: String,
    pub choices: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Vote {
    pub user_id: u64,
    pub choice: String,
}

#[derive(Debug, Clone)]
pub struct RecurringSeries {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub schedule: String,
    pub timezone: Tz,
    pub choices: Vec<String>,
    pub channel_id: u64,
    pub created_by: u64,
    pub next_post_at: DateTime<Utc>,
}

impl RecurringSeries {
    pub fn new(
        title: String,
        description: Option<String>,
        schedule: String,
        timezone: Tz,
        choices: Vec<String>,
        channel_id: u64,
        created_by: u64,
        next_post_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: short_id(),
            title,
            description,
            schedule,
            timezone,
            choices,
            channel_id,
            created_by,
            next_post_at,
        }
    }
}

fn short_id() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}
