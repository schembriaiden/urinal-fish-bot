use anyhow::Result;
use chrono::Utc;
use poise::serenity_prelude::{
    Context, CreateAllowedMentions, CreateMessage, Message, MessageType, UserId,
};
use tracing::{error, info, warn};

use crate::Data;
use crate::easter_egg;

pub async fn handle_message(ctx: &Context, data: &Data, message: &Message) -> Result<()> {
    maybe_send_easter_egg(ctx, data, message).await
}

async fn maybe_send_easter_egg(ctx: &Context, data: &Data, message: &Message) -> Result<()> {
    if message.author.bot || message.kind != MessageType::Regular {
        return Ok(());
    }

    let Some(settings) = data.store.easter_egg_settings().await? else {
        return Ok(());
    };
    if !settings.enabled
        || message.author.id.get() != settings.target_user_id
        || message.channel_id.get() != settings.channel_id
    {
        return Ok(());
    }

    let run_date = easter_egg::trigger_date(Utc::now(), data.config.default_timezone).to_string();
    if data.store.easter_egg_run_exists(&run_date).await? {
        return Ok(());
    }

    let messages = data.store.list_easter_egg_messages().await?;
    let message_pool = messages
        .into_iter()
        .map(|message| message.message)
        .collect::<Vec<_>>();
    let Some(taunt) = easter_egg::choose_message(&message_pool) else {
        warn!("easter egg triggered for {run_date}, but no messages were configured");
        return Ok(());
    };

    let now = Utc::now();
    let claimed = data
        .store
        .record_easter_egg_trigger(
            &run_date,
            settings.target_user_id,
            message.channel_id.get(),
            &taunt,
            now,
        )
        .await?;
    if !claimed {
        return Ok(());
    }

    let content = easter_egg::format_taunt_message(settings.target_user_id, &taunt);
    match message
        .channel_id
        .send_message(
            &ctx.http,
            CreateMessage::new()
                .content(content)
                .allowed_mentions(easter_allowed_mentions(settings.target_user_id)),
        )
        .await
    {
        Ok(_) => {
            info!(
                run_date = %run_date,
                channel_id = message.channel_id.get(),
                target_user_id = settings.target_user_id,
                "posted message-triggered easter egg taunt"
            );
        }
        Err(err) => {
            error!(
                "failed to post message-triggered easter egg taunt for {run_date} in channel {}: {err:?}",
                message.channel_id.get()
            );
        }
    }

    Ok(())
}

fn easter_allowed_mentions(target_user_id: u64) -> CreateAllowedMentions {
    CreateAllowedMentions::new()
        .users([UserId::new(target_user_id)])
        .everyone(false)
        .replied_user(false)
}
