use std::{env, time::Duration};

use anyhow::{Context, Result};
use chrono_tz::Tz;
use poise::serenity_prelude::{ChannelId, GuildId};

#[derive(Clone)]
pub struct Config {
    pub token: String,
    pub guild_id: GuildId,
    pub channel_id: ChannelId,
    pub database_path: String,
    pub default_timezone: Tz,
    pub scheduler_interval: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let token = env::var("DISCORD_TOKEN").context("DISCORD_TOKEN is required")?;
        let guild_id = env_id("DISCORD_GUILD_ID").context("DISCORD_GUILD_ID is required")?;
        let channel_id = env_id("DISCORD_CHANNEL_ID").context("DISCORD_CHANNEL_ID is required")?;
        let database_path = env::var("DATABASE_PATH").unwrap_or_else(|_| "data/bot.db".to_string());
        let default_timezone = env::var("DEFAULT_TIMEZONE")
            .unwrap_or_else(|_| "Europe/Malta".to_string())
            .parse()
            .context("DEFAULT_TIMEZONE must be an IANA timezone like Europe/Malta")?;
        let scheduler_interval = env::var("SCHEDULER_INTERVAL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(60));

        Ok(Self {
            token,
            guild_id: GuildId::new(guild_id),
            channel_id: ChannelId::new(channel_id),
            database_path,
            default_timezone,
            scheduler_interval,
        })
    }
}

fn env_id(name: &str) -> Result<u64> {
    Ok(env::var(name)?.parse::<u64>()?)
}
