use std::sync::Arc;

use anyhow::{Context as AnyhowContext, Result};
use chrono::Utc;
use poise::serenity_prelude::{ChannelId, CreateMessage, Http};
use tokio::time::sleep;
use tracing::{error, info};

use crate::Data;
use crate::discord::{format_discord_time, render_poll_message};
use crate::easter_egg;
use crate::models::{NewPoll, Poll};
use crate::recurrence::next_occurrence;

pub async fn run(data: Arc<Data>, http: Arc<Http>) {
    loop {
        if let Err(err) = tick(&data, &http).await {
            error!("scheduler tick failed: {err:?}");
        }
        sleep(data.config.scheduler_interval).await;
    }
}

async fn tick(data: &Data, http: &Arc<Http>) -> Result<()> {
    let now = Utc::now();
    let due_series = data.store.due_series(Utc::now()).await?;
    for series in due_series {
        let poll = Poll::new(NewPoll {
            title: series.title.clone(),
            description: series.description.clone(),
            when: Some(format!(
                "{} recurrence for {}",
                series.schedule,
                format_discord_time(series.next_post_at)
            )),
            choices: series.choices.clone(),
            channel_id: series.channel_id,
            recurring_id: Some(series.id.clone()),
            created_by: series.created_by,
            created_by_name: series
                .created_by_name
                .clone()
                .unwrap_or_else(|| series.created_by.to_string()),
        });
        data.store.insert_poll(&poll).await?;

        match ChannelId::new(series.channel_id)
            .send_message(
                http,
                render_poll_message(&poll, &[], series.notification.as_ref()),
            )
            .await
        {
            Ok(message) => {
                data.store
                    .set_poll_message(&poll.id, message.id.get())
                    .await?;
                info!(
                    series_id = %series.id,
                    poll_id = %poll.id,
                    channel_id = series.channel_id,
                    message_id = message.id.get(),
                    "posted recurring poll"
                );
            }
            Err(err) => {
                error!("failed to post recurring poll {}: {err:?}", series.id);
                continue;
            }
        }

        let next_post_at = next_occurrence(&series.schedule, series.timezone, Utc::now())
            .with_context(|| format!("invalid stored schedule for {}", series.id))?;
        data.store
            .update_series_next_post(&series.id, next_post_at)
            .await?;
    }

    roll_easter_egg_if_due(data).await?;
    send_due_easter_egg_taunts(data, http, now).await?;

    Ok(())
}

async fn roll_easter_egg_if_due(data: &Data) -> Result<()> {
    let Some(date) = easter_egg::roll_cutoff_date(Utc::now(), data.config.default_timezone) else {
        return Ok(());
    };
    let run_date = date.to_string();
    if data.store.easter_egg_run_exists(&run_date).await? {
        return Ok(());
    }

    let Some(settings) = data.store.easter_egg_settings().await? else {
        return Ok(());
    };
    if !settings.enabled {
        return Ok(());
    }

    let roll = easter_egg::roll_d20();
    if !easter_egg::is_winning_roll(roll) {
        data.store
            .record_easter_egg_roll(&run_date, roll, None, None, None, None)
            .await?;
        info!("easter egg roll for {run_date}: {roll}");
        return Ok(());
    }

    let messages = data.store.list_easter_egg_messages().await?;
    let message_pool = messages
        .into_iter()
        .map(|message| message.message)
        .collect::<Vec<_>>();
    let Some(message) = easter_egg::choose_message(&message_pool) else {
        data.store
            .record_easter_egg_roll(&run_date, roll, None, None, None, None)
            .await?;
        info!("easter egg roll for {run_date}: {roll}, but no messages were configured");
        return Ok(());
    };
    let scheduled_at = easter_egg::random_scheduled_at(
        date,
        settings.window_start_minute,
        settings.window_end_minute,
        data.config.default_timezone,
    )?;

    data.store
        .record_easter_egg_roll(
            &run_date,
            roll,
            Some(scheduled_at),
            Some(settings.target_user_id),
            Some(settings.channel_id),
            Some(&message),
        )
        .await?;
    info!("easter egg roll for {run_date}: {roll}, scheduled at {scheduled_at}");

    Ok(())
}

async fn send_due_easter_egg_taunts(
    data: &Data,
    http: &Arc<Http>,
    now: chrono::DateTime<Utc>,
) -> Result<()> {
    let taunts = data.store.due_easter_egg_taunts(now).await?;
    for taunt in taunts {
        let content = format!("<@{}> {}", taunt.target_user_id, taunt.message);
        match ChannelId::new(taunt.channel_id)
            .send_message(http, CreateMessage::new().content(content))
            .await
        {
            Ok(_) => {
                data.store
                    .mark_easter_egg_sent(&taunt.run_date, Utc::now())
                    .await?;
                info!(
                    run_date = %taunt.run_date,
                    channel_id = taunt.channel_id,
                    target_user_id = taunt.target_user_id,
                    "posted easter egg taunt"
                );
            }
            Err(err) => {
                error!(
                    "failed to post easter egg taunt for {} in channel {}: {err:?}",
                    taunt.run_date, taunt.channel_id
                );
            }
        }
    }

    Ok(())
}
