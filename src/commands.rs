use anyhow::{Context as AnyhowContext, anyhow};
use chrono::Utc;
use poise::serenity_prelude::User;

use crate::choices::{default_choices, parse_choices};
use crate::discord::{format_discord_time, render_poll_message};
use crate::easter_egg;
use crate::models::{
    EasterEggMessage, EasterEggSettings, NewRecurringSeries, Poll, RecurringSeries,
};
use crate::recurrence::next_occurrence;
use crate::validation;
use crate::{Context, Error};

pub fn commands() -> Vec<poise::Command<crate::Data, Error>> {
    vec![
        event(),
        recurring(),
        choice_save(),
        choice_list(),
        choice_delete(),
        series_list(),
        series_delete(),
        easter_set(),
        easter_add_message(),
        easter_status(),
        easter_disable(),
    ]
}

#[poise::command(slash_command)]
pub async fn event(
    ctx: Context<'_>,
    #[description = "Event title"] title: String,
    #[description = "When this is happening"] when: Option<String>,
    #[description = "Extra details"] description: Option<String>,
    #[description = "Comma-separated choices, for example: yes,no,maybe"] choices: Option<String>,
    #[description = "Saved choice set name from /choice_list"] template: Option<String>,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let channel_id = ctx.channel_id();
    let title = validation::poll_title(title)?;
    let description = validation::optional_description(description)?;
    let when = validation::optional_when(when)?;
    let template = template.map(validation::template_name).transpose()?;
    let choices = resolve_choices(ctx, choices, template).await?;
    let poll = Poll::new(
        title,
        description,
        when,
        choices,
        channel_id.get(),
        None,
        ctx.author().id.get(),
    );

    ctx.data().store.insert_poll(&poll).await?;
    let message = ctx
        .channel_id()
        .send_message(
            &ctx.serenity_context().http,
            render_poll_message(&poll, &[]),
        )
        .await
        .context("failed to send poll message")?;
    ctx.data()
        .store
        .set_poll_message(&poll.id, message.id.get())
        .await?;

    reply_ephemeral(
        ctx,
        format!(
            "Created event poll: https://discord.com/channels/{}/{}/{}",
            ctx.data().config.guild_id,
            channel_id,
            message.id
        ),
    )
    .await
}

#[poise::command(slash_command)]
pub async fn recurring(
    ctx: Context<'_>,
    #[description = "Event title"] title: String,
    #[description = "daily 19:00, weekly fri 20:00, or monthly 15 19:30"] schedule: String,
    #[description = "Extra details"] description: Option<String>,
    #[description = "Comma-separated choices, for example: yes,no,maybe"] choices: Option<String>,
    #[description = "Saved choice set name from /choice_list"] template: Option<String>,
    #[description = "IANA timezone, default comes from DEFAULT_TIMEZONE"] timezone: Option<String>,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let channel_id = ctx.channel_id();
    let title = validation::poll_title(title)?;
    let description = validation::optional_description(description)?;
    let template = template.map(validation::template_name).transpose()?;
    let timezone = timezone
        .map(|value| value.parse())
        .transpose()
        .context("timezone must be an IANA timezone like Europe/Malta")?
        .unwrap_or(ctx.data().config.default_timezone);
    let choices = resolve_choices(ctx, choices, template).await?;
    let next_post_at = next_occurrence(&schedule, timezone, Utc::now())
        .with_context(|| format!("could not parse schedule '{schedule}'"))?;

    let series = RecurringSeries::new(NewRecurringSeries {
        title,
        description,
        schedule,
        timezone,
        choices,
        channel_id: channel_id.get(),
        created_by: ctx.author().id.get(),
        next_post_at,
    });
    ctx.data().store.insert_series(&series).await?;

    reply_ephemeral(
        ctx,
        format!(
            "Saved recurring event `{}`. First poll posts at {}.",
            series.title,
            format_discord_time(next_post_at)
        ),
    )
    .await
}

#[poise::command(slash_command)]
pub async fn choice_save(
    ctx: Context<'_>,
    #[description = "Name for this choice set"] name: String,
    #[description = "Comma-separated choices, for example: yes,no,maybe"] choices: String,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let name = validation::template_name(name)?;
    let choices = parse_choices(&choices)?;
    ctx.data()
        .store
        .save_template(&name, &choices, ctx.author().id.get())
        .await?;

    reply_ephemeral(ctx, format!("Saved `{name}` as: {}", choices.join(", "))).await
}

#[poise::command(slash_command)]
pub async fn choice_list(ctx: Context<'_>) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let templates = ctx.data().store.list_templates().await?;
    let body = if templates.is_empty() {
        "No saved choice sets yet. Use `/choice_save` to create one.".to_string()
    } else {
        templates
            .into_iter()
            .map(|template| format!("`{}`: {}", template.name, template.choices.join(", ")))
            .collect::<Vec<_>>()
            .join("\n")
    };

    reply_ephemeral(ctx, body).await
}

#[poise::command(slash_command)]
pub async fn choice_delete(
    ctx: Context<'_>,
    #[description = "Choice set name"] name: String,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let name = validation::template_name(name)?;
    let deleted = ctx.data().store.delete_template(&name).await?;
    let message = if deleted {
        format!("Deleted choice set `{name}`.")
    } else {
        format!("I could not find a choice set named `{name}`.")
    };

    reply_ephemeral(ctx, message).await
}

#[poise::command(slash_command)]
pub async fn series_list(ctx: Context<'_>) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let series = ctx.data().store.list_active_series().await?;
    let body = if series.is_empty() {
        "No recurring event series are active.".to_string()
    } else {
        series
            .into_iter()
            .map(|item| {
                format!(
                    "`{}`: {} | next {} | choices: {}",
                    item.id,
                    item.title,
                    format_discord_time(item.next_post_at),
                    item.choices.join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    reply_ephemeral(ctx, body).await
}

#[poise::command(slash_command)]
pub async fn series_delete(
    ctx: Context<'_>,
    #[description = "Series ID from /series_list"] id: String,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let deleted = ctx.data().store.deactivate_series(&id).await?;
    let message = if deleted {
        format!("Stopped recurring series `{id}`.")
    } else {
        format!("I could not find an active recurring series named `{id}`.")
    };

    reply_ephemeral(ctx, message).await
}

#[poise::command(
    slash_command,
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn easter_set(
    ctx: Context<'_>,
    #[description = "User to target"] target: User,
    #[description = "Earliest send time, HH:MM, 04:00 or later"] start_time: String,
    #[description = "Latest send time, HH:MM"] end_time: String,
    #[description = "First message to add to the pool"] message: String,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let start_minute = easter_egg::parse_time(&start_time, "start_time")?;
    let end_minute = easter_egg::parse_time(&end_time, "end_time")?;
    easter_egg::validate_window(start_minute, end_minute)?;
    let message = validation::clean_text(message, "easter egg message", 200)?;

    let settings = EasterEggSettings {
        enabled: true,
        target_user_id: target.id.get(),
        channel_id: ctx.channel_id().get(),
        window_start_minute: start_minute,
        window_end_minute: end_minute,
        updated_by: ctx.author().id.get(),
    };
    ctx.data()
        .store
        .upsert_easter_egg_settings(&settings)
        .await?;
    ctx.data()
        .store
        .add_easter_egg_message(&EasterEggMessage::new(message), ctx.author().id.get())
        .await?;

    reply_ephemeral(
        ctx,
        format!(
            "Easter egg enabled for <@{}> in <#{}>. Daily roll is at 04:00; on an 11, I will post between {} and {}.",
            settings.target_user_id,
            settings.channel_id,
            easter_egg::format_time(settings.window_start_minute),
            easter_egg::format_time(settings.window_end_minute)
        ),
    )
    .await
}

#[poise::command(
    slash_command,
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn easter_add_message(
    ctx: Context<'_>,
    #[description = "Message to add to the easter egg pool"] message: String,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let Some(settings) = ctx.data().store.easter_egg_settings().await? else {
        return reply_ephemeral(ctx, "Use `/easter_set` first.".to_string()).await;
    };
    if !settings.enabled {
        return reply_ephemeral(
            ctx,
            "The easter egg is disabled. Use `/easter_set` first.".to_string(),
        )
        .await;
    }

    let message = validation::clean_text(message, "easter egg message", 200)?;
    ctx.data()
        .store
        .add_easter_egg_message(&EasterEggMessage::new(message), ctx.author().id.get())
        .await?;

    reply_ephemeral(ctx, "Added easter egg message.".to_string()).await
}

#[poise::command(
    slash_command,
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn easter_status(ctx: Context<'_>) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let Some(settings) = ctx.data().store.easter_egg_settings().await? else {
        return reply_ephemeral(ctx, "Easter egg is not configured.".to_string()).await;
    };
    let messages = ctx.data().store.list_easter_egg_messages().await?;
    let status = if settings.enabled {
        "enabled"
    } else {
        "disabled"
    };
    let preview = messages
        .iter()
        .take(5)
        .map(|message| format!("- `{}`", message.message))
        .collect::<Vec<_>>()
        .join("\n");
    let preview = if preview.is_empty() {
        "No messages configured.".to_string()
    } else {
        preview
    };

    reply_ephemeral(
        ctx,
        format!(
            "Easter egg is {status}.\nTarget: <@{}>\nChannel: <#{}>\nWindow: {}-{}\nMessages: {}\n{}",
            settings.target_user_id,
            settings.channel_id,
            easter_egg::format_time(settings.window_start_minute),
            easter_egg::format_time(settings.window_end_minute),
            messages.len(),
            preview
        ),
    )
    .await
}

#[poise::command(
    slash_command,
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn easter_disable(ctx: Context<'_>) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let disabled = ctx.data().store.disable_easter_egg().await?;
    let message = if disabled {
        "Easter egg disabled."
    } else {
        "Easter egg is not configured."
    };

    reply_ephemeral(ctx, message.to_string()).await
}

async fn resolve_choices(
    ctx: Context<'_>,
    choices: Option<String>,
    template: Option<String>,
) -> Result<Vec<String>, Error> {
    if let Some(raw_choices) = choices {
        return parse_choices(&raw_choices);
    }

    if let Some(template) = template {
        let Some(template) = ctx.data().store.get_template(&template).await? else {
            return Err(anyhow!("choice template `{template}` was not found"));
        };
        return Ok(template.choices);
    }

    Ok(default_choices())
}

async fn ensure_allowed_channel(ctx: Context<'_>) -> Result<(), Error> {
    if ctx
        .data()
        .config
        .channel_ids
        .iter()
        .any(|channel_id| *channel_id == ctx.channel_id())
    {
        return Ok(());
    }

    let channels = ctx
        .data()
        .config
        .channel_ids
        .iter()
        .map(|channel_id| format!("<#{channel_id}>"))
        .collect::<Vec<_>>()
        .join(", ");
    reply_ephemeral(
        ctx,
        format!("Use this bot in {channels}. I will ignore commands anywhere else."),
    )
    .await?;
    Err(anyhow!(
        "command used outside allowed channels: {}",
        ctx.data()
            .config
            .channel_ids
            .iter()
            .map(|channel_id| channel_id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

async fn reply_ephemeral(ctx: Context<'_>, content: String) -> Result<(), Error> {
    ctx.send(
        poise::CreateReply::default()
            .content(content)
            .ephemeral(true),
    )
    .await?;
    Ok(())
}
