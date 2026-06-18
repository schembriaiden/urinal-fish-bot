use anyhow::{Context as AnyhowContext, anyhow};
use chrono::Utc;
use poise::serenity_prelude::{CreateAllowedMentions, CreateMessage, User, UserId};
use tracing::{info, warn};

use crate::choices::{default_choices, parse_choices};
use crate::discord::{format_discord_time, render_poll_message};
use crate::easter_egg;
use crate::models::{
    EasterEggMessage, EasterEggSettings, NewPoll, NewRecurringSeries, Poll, PollNotification,
    RecurringSeries,
};
use crate::recurrence::next_occurrence;
use crate::validation;
use crate::{Context, Error};

pub fn commands() -> Vec<poise::Command<crate::Data, Error>> {
    vec![
        help(),
        event(),
        series_list(),
        series_delete(),
        easter_set(),
        easter_add_message(),
        easter_delete_message(),
        easter_status(),
        easter_test(),
        easter_disable(),
    ]
}

/// Show Urinal Fish help.
#[poise::command(slash_command)]
pub async fn help(ctx: Context<'_>) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    reply_ephemeral(ctx, help_text()).await
}

/// Create event polls.
#[poise::command(slash_command, subcommands("single", "recurring"), subcommand_required)]
pub async fn event(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Create a one-off event poll.
#[poise::command(slash_command)]
pub async fn single(
    ctx: Context<'_>,
    #[description = "Event title"] title: String,
    #[description = "When this is happening"] when: String,
    #[description = "Comma-separated choices, for example: yes,no,maybe"]
    #[autocomplete = "autocomplete_choices"]
    choices: String,
    #[description = "Where this is happening"]
    #[rename = "where"]
    where_location: Option<String>,
    #[description = "Extra details"] description: Option<String>,
    #[description = "Optional users or roles to ping, for example: @friends @person"]
    notify: Option<String>,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let channel_id = ctx.channel_id();
    let title = validation::poll_title(title)?;
    let description = validation::optional_description(description)?;
    let when = validation::optional_when(Some(when))?;
    let location = validation::optional_location(where_location)?;
    let choices = parse_choices(&choices)?;
    let notification = parse_notification(notify)?;
    ctx.data().store.record_choice_history(&choices).await?;
    let poll = Poll::new(NewPoll {
        title,
        description,
        when,
        location,
        choices,
        channel_id: channel_id.get(),
        recurring_id: None,
        created_by: ctx.author().id.get(),
        created_by_name: display_name_for_user(ctx.author()),
    });

    ctx.data().store.insert_poll(&poll).await?;
    let message = ctx
        .channel_id()
        .send_message(
            &ctx.serenity_context().http,
            render_poll_message(
                &poll,
                &[],
                notification.as_ref(),
                ctx.data().config.default_timezone,
            ),
        )
        .await
        .context("failed to send poll message")?;
    ctx.data()
        .store
        .set_poll_message(&poll.id, message.id.get())
        .await?;
    info!(
        poll_id = %poll.id,
        channel_id = channel_id.get(),
        message_id = message.id.get(),
        created_by = ctx.author().id.get(),
        "created event poll"
    );

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

/// Create a recurring event series.
#[poise::command(slash_command)]
pub async fn recurring(
    ctx: Context<'_>,
    #[description = "Event title"] title: String,
    #[description = "daily 19:00, weekly fri 20:00, or monthly 15 19:30"] schedule: String,
    #[description = "Event date/time shown in the poll, for example: Friday 20:30"] when: String,
    #[description = "Comma-separated choices, for example: yes,no,maybe"]
    #[autocomplete = "autocomplete_choices"]
    choices: String,
    #[description = "Where this recurring event happens"]
    #[rename = "where"]
    where_location: Option<String>,
    #[description = "Optional users or roles to ping whenever the recurring poll posts"]
    notify: Option<String>,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let channel_id = ctx.channel_id();
    let title = validation::poll_title(title)?;
    let when =
        validation::optional_when(Some(when))?.ok_or_else(|| anyhow!("when cannot be empty"))?;
    let location = validation::optional_location(where_location)?;
    let timezone = ctx.data().config.default_timezone;
    let choices = parse_choices(&choices)?;
    let notification = parse_notification(notify)?;
    ctx.data().store.record_choice_history(&choices).await?;
    let next_post_at = next_occurrence(&schedule, timezone, Utc::now())
        .with_context(|| format!("could not parse schedule '{schedule}'"))?;

    let series = RecurringSeries::new(NewRecurringSeries {
        title,
        description: None,
        schedule,
        when,
        location,
        timezone,
        choices,
        notification,
        channel_id: channel_id.get(),
        created_by: ctx.author().id.get(),
        created_by_name: display_name_for_user(ctx.author()),
        next_post_at,
    });
    ctx.data().store.insert_series(&series).await?;
    info!(
        series_id = %series.id,
        channel_id = channel_id.get(),
        created_by = ctx.author().id.get(),
        next_post_at = %series.next_post_at,
        "created recurring event series"
    );

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
                    "`{}`: {} | when: {} | next post {} | choices: {} | notify: {}",
                    item.id,
                    item.title,
                    series_when_summary(&item.when),
                    format_discord_time(item.next_post_at),
                    item.choices.join(", "),
                    series_notification_summary(item.notification.as_ref())
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

/// Configure the easter egg target, posting window, and first message.
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
    info!(
        target_user_id = settings.target_user_id,
        channel_id = settings.channel_id,
        updated_by = settings.updated_by,
        "configured easter egg"
    );

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

/// Add another possible easter egg message to the random message pool.
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
    info!(
        created_by = ctx.author().id.get(),
        "added easter egg message"
    );

    reply_ephemeral(ctx, "Added easter egg message.".to_string()).await
}

/// Delete one easter egg message by ID from `/easter_status`.
#[poise::command(
    slash_command,
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn easter_delete_message(
    ctx: Context<'_>,
    #[description = "Message ID shown in /easter_status"] id: String,
) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let id = validation::clean_text(id, "easter egg message id", 32)?;
    let deleted = ctx.data().store.delete_easter_egg_message(&id).await?;
    info!(
        message_id = %id,
        deleted,
        deleted_by = ctx.author().id.get(),
        "deleted easter egg message"
    );

    let response = if deleted {
        format!("Deleted easter egg message `{id}`.")
    } else {
        format!("I could not find an easter egg message with ID `{id}`.")
    };

    reply_ephemeral(ctx, response).await
}

/// Show whether the easter egg is enabled, who it targets, and its messages.
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
        .map(|message| format!("- `{}`: {}", message.id, message.message))
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

/// Send one easter egg message immediately in this channel for testing.
#[poise::command(
    slash_command,
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn easter_test(ctx: Context<'_>) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let Some(settings) = ctx.data().store.easter_egg_settings().await? else {
        return reply_ephemeral(ctx, "Use `/easter_set` first.".to_string()).await;
    };
    let messages = ctx.data().store.list_easter_egg_messages().await?;
    let message_pool = messages
        .into_iter()
        .map(|message| message.message)
        .collect::<Vec<_>>();
    let Some(message) = easter_egg::choose_message(&message_pool) else {
        return reply_ephemeral(
            ctx,
            "No easter egg messages are configured. Use `/easter_add_message` first.".to_string(),
        )
        .await;
    };

    let content = easter_egg::format_taunt_message(settings.target_user_id, &message);
    ctx.channel_id()
        .send_message(
            &ctx.serenity_context().http,
            CreateMessage::new()
                .content(content)
                .allowed_mentions(easter_test_allowed_mentions(settings.target_user_id)),
        )
        .await
        .context("failed to send easter egg test message")?;

    info!(
        target_user_id = settings.target_user_id,
        channel_id = ctx.channel_id().get(),
        tested_by = ctx.author().id.get(),
        "sent easter egg test message"
    );

    reply_ephemeral(ctx, "Sent easter egg test message.".to_string()).await
}

/// Disable the easter egg without deleting its configured messages.
#[poise::command(
    slash_command,
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn easter_disable(ctx: Context<'_>) -> Result<(), Error> {
    ensure_allowed_channel(ctx).await?;

    let disabled = ctx.data().store.disable_easter_egg().await?;
    info!(
        disabled,
        updated_by = ctx.author().id.get(),
        "disabled easter egg"
    );
    let message = if disabled {
        "Easter egg disabled."
    } else {
        "Easter egg is not configured."
    };

    reply_ephemeral(ctx, message.to_string()).await
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
    warn!(
        channel_id = ctx.channel_id().get(),
        user_id = ctx.author().id.get(),
        "rejected command outside allowed channels"
    );
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

fn parse_notification(value: Option<String>) -> Result<Option<PollNotification>, Error> {
    let Some(value) = value.map(|value| value.trim().to_string()) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }

    let tokens = value
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut user_ids = Vec::new();
    let mut role_ids = Vec::new();

    for token in tokens {
        if let Some(user_id) = parse_user_mention(token) {
            if !user_ids.contains(&user_id) {
                user_ids.push(user_id);
            }
            continue;
        }
        if let Some(role_id) = parse_role_mention(token) {
            if !role_ids.contains(&role_id) {
                role_ids.push(role_id);
            }
            continue;
        }
        return Err(anyhow!(
            "notify can only contain user or role mentions, like @person or @friends"
        ));
    }

    if user_ids.is_empty() && role_ids.is_empty() {
        return Ok(None);
    }

    Ok(Some(PollNotification {
        content: user_ids
            .iter()
            .map(|user_id| format!("<@{user_id}>"))
            .chain(role_ids.iter().map(|role_id| format!("<@&{role_id}>")))
            .collect::<Vec<_>>()
            .join(" "),
        user_ids,
        role_ids,
    }))
}

fn parse_user_mention(token: &str) -> Option<u64> {
    let token = token.strip_prefix("<@")?.strip_suffix('>')?;
    let token = token.strip_prefix('!').unwrap_or(token);
    if token.starts_with('&') {
        return None;
    }
    token.parse().ok()
}

fn parse_role_mention(token: &str) -> Option<u64> {
    token.strip_prefix("<@&")?.strip_suffix('>')?.parse().ok()
}

fn display_name_for_user(user: &User) -> String {
    user.global_name
        .as_deref()
        .unwrap_or(&user.name)
        .to_string()
}

fn series_notification_summary(notification: Option<&PollNotification>) -> String {
    notification
        .map(|notification| notification.content.clone())
        .filter(|content| !content.trim().is_empty())
        .unwrap_or_else(|| "none".to_string())
}

fn series_when_summary(when: &str) -> &str {
    let when = when.trim();
    if when.is_empty() { "not set" } else { when }
}

fn easter_test_allowed_mentions(target_user_id: u64) -> CreateAllowedMentions {
    CreateAllowedMentions::new()
        .users([UserId::new(target_user_id)])
        .everyone(false)
        .replied_user(false)
}

fn help_text() -> String {
    [
        "**Urinal Fish help**",
        "",
        "**Create a one-off poll**",
        "`/event single title: Drinks when: Friday 20:00 choices: yes,no,maybe where: Berlin`",
        "",
        "**Create a recurring poll**",
        "`/event recurring title: Friday drinks schedule: weekly fri 12:00 when: Friday 20:00 choices: yes,no,maybe where: Berlin`",
        "",
        "**Useful options**",
        "`choices` is comma-separated and required. Previously used choice sets are suggested while typing.",
        "`where` and `notify` are optional. One-off polls also support `description`. `notify` can ping users or roles.",
        "",
        "**Recurring schedules**",
        "`daily 19:00`, `weekly fri 20:00`, `friday 20:00`, `monthly 15 19:30`",
        "",
        "**Manage recurring events**",
        "`/series_list` shows active recurring events.",
        "`/series_delete id:<series id>` stops one.",
        "",
        "**Voting**",
        "Press a choice button to vote. Pressing a different button changes your vote.",
    ]
    .join("\n")
}

async fn autocomplete_choices(ctx: Context<'_>, partial: &str) -> Vec<String> {
    let mut suggestions = vec![default_choices().join(", ")];

    match ctx.data().store.recent_choice_sets(partial, 24).await {
        Ok(history) => suggestions.extend(history),
        Err(err) => warn!("failed to load choice autocomplete suggestions: {err:?}"),
    }

    let partial = partial.trim().to_lowercase();
    suggestions
        .into_iter()
        .filter(|choice_set| partial.is_empty() || choice_set.to_lowercase().contains(&partial))
        .fold(Vec::new(), |mut unique, choice_set| {
            if unique.iter().all(|existing| existing != &choice_set) {
                unique.push(choice_set);
            }
            unique
        })
        .into_iter()
        .take(25)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notification_mentions() {
        let notification = parse_notification(Some("<@123> <@&456>".to_string()))
            .unwrap()
            .unwrap();

        assert_eq!(notification.content, "<@123> <@&456>");
        assert_eq!(notification.user_ids, [123]);
        assert_eq!(notification.role_ids, [456]);
    }

    #[test]
    fn rejects_non_mention_notification_text() {
        let error = parse_notification(Some("@everyone hello".to_string()))
            .unwrap_err()
            .to_string();

        assert!(error.contains("user or role mentions"));
    }

    #[test]
    fn summarizes_series_notifications() {
        let notification = PollNotification {
            content: "<@123> <@&456>".to_string(),
            user_ids: vec![123],
            role_ids: vec![456],
        };

        assert_eq!(
            series_notification_summary(Some(&notification)),
            "<@123> <@&456>"
        );
        assert_eq!(series_notification_summary(None), "none");
    }

    #[test]
    fn summarizes_empty_series_when() {
        assert_eq!(series_when_summary("Friday 20:00"), "Friday 20:00");
        assert_eq!(series_when_summary("   "), "not set");
    }
}
