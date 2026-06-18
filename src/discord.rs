use chrono::{DateTime, Utc};
use poise::serenity_prelude::{
    ButtonStyle, CreateActionRow, CreateAllowedMentions, CreateButton, CreateEmbed,
    CreateEmbedFooter, CreateMessage, RoleId, UserId,
};

use crate::models::{Poll, PollNotification, Vote};

pub fn render_poll_message(
    poll: &Poll,
    responses: &[Vote],
    notification: Option<&PollNotification>,
) -> CreateMessage {
    let mut message = CreateMessage::new()
        .embed(render_poll_embed(poll, responses))
        .components(render_poll_buttons(poll));

    if let Some(notification) = notification {
        message = message
            .content(&notification.content)
            .allowed_mentions(notification_allowed_mentions(notification));
    }

    message
}

pub fn render_poll_embed(poll: &Poll, responses: &[Vote]) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .color(0x2f9eaa)
        .title(format!("📅 {}", embed_text(&poll.title).to_uppercase()));

    if let Some(description) = &poll.description {
        embed = embed.description(embed_text(description));
    }

    embed = embed.field(
        "🗓 When",
        embed_text(poll.when.as_deref().unwrap_or("Not specified")),
        true,
    );

    if let Some(location) = filled_text(poll.location.as_deref()) {
        embed = embed.field("📍 Where", embed_text(location), true).field(
            "👤 Creator",
            user_mention(poll.created_by),
            true,
        );
    } else {
        embed = embed
            .field("👤 Creator", user_mention(poll.created_by), true)
            .field(row_break_name(), row_break_value(), false);
    }

    for choice in &poll.choices {
        embed = embed.field(
            choice_field_name(choice, responses),
            choice_field_value(choice, responses),
            true,
        );
    }

    embed.footer(CreateEmbedFooter::new(format!(
        "Event ID: {} • Created on: {}",
        poll.id,
        poll.created_at.format("%-d %B %Y at %H:%M")
    )))
}

pub fn render_poll_buttons(poll: &Poll) -> Vec<CreateActionRow> {
    poll.choices
        .chunks(5)
        .enumerate()
        .map(|(chunk_index, chunk)| {
            let buttons = chunk
                .iter()
                .enumerate()
                .map(|(index, choice)| {
                    let choice_index = chunk_index * 5 + index;
                    CreateButton::new(format!("vote:{}:{}", poll.id, choice_index))
                        .label(choice)
                        .style(button_style(choice))
                })
                .collect();
            CreateActionRow::Buttons(buttons)
        })
        .collect()
}

fn choice_field_name(choice: &str, responses: &[Vote]) -> String {
    let count = responses
        .iter()
        .filter(|vote| vote.choice == choice)
        .count();
    format!("{} - {}", embed_text(choice), vote_count(count))
}

fn choice_field_value(choice: &str, responses: &[Vote]) -> String {
    let voters = responses
        .iter()
        .filter(|vote| vote.choice == choice)
        .map(|vote| user_mention(vote.user_id))
        .collect::<Vec<_>>();

    if voters.is_empty() {
        return "_No one yet_".to_string();
    }

    let value = voters.join("\n");
    if value.len() <= 1024 {
        return value;
    }

    let mut trimmed = String::new();
    for voter in voters {
        if trimmed.len() + voter.len() + 1 > 1000 {
            break;
        }
        if !trimmed.is_empty() {
            trimmed.push('\n');
        }
        trimmed.push_str(&voter);
    }
    format!("{trimmed}\n…and more")
}

fn vote_count(count: usize) -> String {
    match count {
        1 => "1 vote".to_string(),
        _ => format!("{count} votes"),
    }
}

fn user_mention(user_id: u64) -> String {
    format!("<@{user_id}>")
}

fn filled_text(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    })
}

fn row_break_name() -> &'static str {
    "\u{200B}"
}

fn row_break_value() -> &'static str {
    "\u{200B}"
}

fn embed_text(value: &str) -> String {
    value
        .replace('`', "'")
        .replace(['\n', '\r', '\t'], " ")
        .trim()
        .to_string()
}

fn notification_allowed_mentions(notification: &PollNotification) -> CreateAllowedMentions {
    CreateAllowedMentions::new()
        .users(notification.user_ids.iter().copied().map(UserId::new))
        .roles(notification.role_ids.iter().copied().map(RoleId::new))
        .everyone(false)
        .replied_user(false)
}

fn button_style(choice: &str) -> ButtonStyle {
    match choice.trim().to_lowercase().as_str() {
        "yes" | "y" | "going" | "in" => ButtonStyle::Success,
        "no" | "n" | "not going" | "out" => ButtonStyle::Danger,
        "maybe" | "later" | "unsure" => ButtonStyle::Secondary,
        _ => ButtonStyle::Primary,
    }
}

pub fn format_discord_time(time: DateTime<Utc>) -> String {
    format!("<t:{}:F>", time.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_clickable_mentions_for_poll_people() {
        let poll = Poll {
            id: "f531adfd".to_string(),
            title: "Tal-Aħħar".to_string(),
            description: Some("issa".to_string()),
            when: Some("Today at 22:30".to_string()),
            location: Some("Berlin".to_string()),
            choices: vec![
                "caru tond".to_string(),
                "bajd u patata".to_string(),
                "flat".to_string(),
            ],
            channel_id: 1,
            message_id: None,
            recurring_id: None,
            created_by: 42,
            created_by_name: Some("EventCreator".to_string()),
            created_at: Utc::now(),
        };
        let votes = vec![
            Vote {
                user_id: 1,
                display_name: Some("FriendOne".to_string()),
                choice: "caru tond".to_string(),
            },
            Vote {
                user_id: 2,
                display_name: Some("EventCreator".to_string()),
                choice: "caru tond".to_string(),
            },
            Vote {
                user_id: 3,
                display_name: Some("FriendTwo".to_string()),
                choice: "bajd u patata".to_string(),
            },
        ];

        assert_eq!(user_mention(poll.created_by), "<@42>");
        assert_eq!(
            choice_field_name("caru tond", &votes),
            "caru tond - 2 votes"
        );
        assert_eq!(
            choice_field_name("bajd u patata", &votes),
            "bajd u patata - 1 vote"
        );
        assert_eq!(choice_field_name("flat", &votes), "flat - 0 votes");
        assert_eq!(choice_field_value("caru tond", &votes), "<@1>\n<@2>");
        assert_eq!(choice_field_value("bajd u patata", &votes), "<@3>");
        assert_eq!(choice_field_value("flat", &votes), "_No one yet_");
    }

    #[test]
    fn skips_where_when_location_is_not_set() {
        assert_eq!(filled_text(None), None);
        assert_eq!(filled_text(Some("   ")), None);
        assert_eq!(filled_text(Some("Berlin")), Some("Berlin"));
    }
}
