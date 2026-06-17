use std::sync::Arc;

use anyhow::{Context as AnyhowContext, Result};
use chrono::Utc;
use poise::serenity_prelude::{ChannelId, Http};
use tokio::time::sleep;
use tracing::error;

use crate::Data;
use crate::discord::{format_discord_time, render_poll_message};
use crate::models::Poll;
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
        let poll = Poll::new(
            series.title.clone(),
            series.description.clone(),
            Some(format!(
                "{} recurrence for {}",
                series.schedule,
                format_discord_time(series.next_post_at)
            )),
            series.choices.clone(),
            series.channel_id,
            Some(series.id.clone()),
            series.created_by,
        );
        data.store.insert_poll(&poll).await?;

        match ChannelId::new(series.channel_id)
            .send_message(http, render_poll_message(&poll, &[]))
            .await
        {
            Ok(message) => {
                data.store
                    .set_poll_message(&poll.id, message.id.get())
                    .await?
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
