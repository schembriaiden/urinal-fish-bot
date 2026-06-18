use std::{env, time::Duration};

use anyhow::{Context, Result};
use chrono_tz::Tz;
use poise::serenity_prelude::{ChannelId, GuildId};

#[derive(Clone)]
pub struct Config {
    pub token: String,
    pub guild_id: GuildId,
    pub channel_ids: Vec<ChannelId>,
    pub database_path: String,
    pub default_timezone: Tz,
    pub scheduler_interval: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let token = env::var("DISCORD_TOKEN").context("DISCORD_TOKEN is required")?;
        let guild_id = env_id("DISCORD_GUILD_ID").context("DISCORD_GUILD_ID is required")?;
        let channel_ids = channel_ids_from_env()?;
        let database_path = env::var("DATABASE_PATH").unwrap_or_else(|_| "data/bot.db".to_string());
        let default_timezone = env::var("DEFAULT_TIMEZONE")
            .unwrap_or_else(|_| "Europe/Berlin".to_string())
            .parse()
            .context("DEFAULT_TIMEZONE must be an IANA timezone like Europe/Berlin")?;
        let scheduler_interval = env::var("SCHEDULER_INTERVAL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(60));

        Ok(Self {
            token,
            guild_id: GuildId::new(guild_id),
            channel_ids,
            database_path,
            default_timezone,
            scheduler_interval,
        })
    }
}

fn env_id(name: &str) -> Result<u64> {
    Ok(env::var(name)?.parse::<u64>()?)
}

fn channel_ids_from_env() -> Result<Vec<ChannelId>> {
    let raw = env::var("DISCORD_CHANNEL_IDS")
        .or_else(|_| env::var("DISCORD_CHANNEL_ID"))
        .context("DISCORD_CHANNEL_IDS or DISCORD_CHANNEL_ID is required")?;
    let channel_ids = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<u64>()
                .map(ChannelId::new)
                .with_context(|| format!("invalid Discord channel ID '{value}'"))
        })
        .collect::<Result<Vec<_>>>()?;

    if channel_ids.is_empty() {
        anyhow::bail!("DISCORD_CHANNEL_IDS must contain at least one channel ID");
    }

    Ok(channel_ids)
}
