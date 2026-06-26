use std::sync::Arc;

use anyhow::{Context as AnyhowContext, Result};
use chrono::Utc;
use poise::serenity_prelude::{ChannelId, Http};
use tokio::time::sleep;
use tracing::{error, info};

use crate::Data;
use crate::discord::render_poll_message;
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
    let due_series = data.store.due_series(Utc::now()).await?;
    for series in due_series {
        let poll = Poll::new(NewPoll {
            title: series.title.clone(),
            description: series.description.clone(),
            when: Some(series_when_text(&series.when, series.next_post_at)),
            location: series.location.clone(),
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
                render_poll_message(
                    &poll,
                    &[],
                    series.notification.as_ref(),
                    data.config.default_timezone,
                ),
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

    Ok(())
}

fn series_when_text(when: &str, next_post_at: chrono::DateTime<Utc>) -> String {
    let when = when.trim();
    if when.is_empty() {
        return next_post_at.format("%A %H:%M").to_string();
    }
    when.to_string()
}
