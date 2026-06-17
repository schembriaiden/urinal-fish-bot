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
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Poll {
    pub fn new(input: NewPoll) -> Self {
        Self {
            id: short_id(),
            title: input.title,
            description: input.description,
            when: input.when,
            choices: input.choices,
            channel_id: input.channel_id,
            message_id: None,
            recurring_id: input.recurring_id,
            created_by: input.created_by,
            created_by_name: Some(input.created_by_name),
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewPoll {
    pub title: String,
    pub description: Option<String>,
    pub when: Option<String>,
    pub choices: Vec<String>,
    pub channel_id: u64,
    pub recurring_id: Option<String>,
    pub created_by: u64,
    pub created_by_name: String,
}

#[derive(Debug, Clone)]
pub struct Vote {
    pub user_id: u64,
    pub display_name: Option<String>,
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
    pub notification: Option<PollNotification>,
    pub channel_id: u64,
    pub created_by: u64,
    pub created_by_name: Option<String>,
    pub next_post_at: DateTime<Utc>,
}

impl RecurringSeries {
    pub fn new(input: NewRecurringSeries) -> Self {
        Self {
            id: short_id(),
            title: input.title,
            description: input.description,
            schedule: input.schedule,
            timezone: input.timezone,
            choices: input.choices,
            notification: input.notification,
            channel_id: input.channel_id,
            created_by: input.created_by,
            created_by_name: Some(input.created_by_name),
            next_post_at: input.next_post_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PollNotification {
    pub content: String,
    pub user_ids: Vec<u64>,
    pub role_ids: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct NewRecurringSeries {
    pub title: String,
    pub description: Option<String>,
    pub schedule: String,
    pub timezone: Tz,
    pub choices: Vec<String>,
    pub notification: Option<PollNotification>,
    pub channel_id: u64,
    pub created_by: u64,
    pub created_by_name: String,
    pub next_post_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EasterEggSettings {
    pub enabled: bool,
    pub target_user_id: u64,
    pub channel_id: u64,
    pub window_start_minute: u16,
    pub window_end_minute: u16,
    pub updated_by: u64,
}

#[derive(Debug, Clone)]
pub struct EasterEggMessage {
    pub id: String,
    pub message: String,
}

impl EasterEggMessage {
    pub fn new(message: String) -> Self {
        Self {
            id: short_id(),
            message,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DueEasterEggTaunt {
    pub run_date: String,
    pub target_user_id: u64,
    pub channel_id: u64,
    pub message: String,
}

fn short_id() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}
