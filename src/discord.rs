use chrono::{DateTime, Utc};
use poise::serenity_prelude::{
    ButtonStyle, CreateActionRow, CreateButton, CreateEmbed, CreateEmbedFooter, CreateMessage,
};

use crate::models::{Poll, Vote};

pub fn render_poll_message(poll: &Poll, responses: &[Vote]) -> CreateMessage {
    CreateMessage::new()
        .embed(render_poll_embed(poll, responses))
        .components(render_poll_buttons(poll, responses))
}

pub fn render_poll_embed(poll: &Poll, responses: &[Vote]) -> CreateEmbed {
    let total_votes = responses.len();
    let mut embed = CreateEmbed::new()
        .title(&poll.title)
        .color(0x2f9eaa)
        .field(
            "When",
            poll.when.as_deref().unwrap_or("Not specified"),
            false,
        )
        .field("Created by", format!("<@{}>", poll.created_by), true)
        .field("Total votes", total_votes.to_string(), true)
        .timestamp(poll.created_at);

    if let Some(description) = &poll.description {
        embed = embed.description(description);
    }

    let mut response_lines = Vec::new();
    for choice in &poll.choices {
        let users = voters_for_choice(responses, choice);
        let count = users.len();
        let value = if users.is_empty() {
            "_Nobody yet_".to_string()
        } else {
            users.join("  ")
        };
        response_lines.push(format!(
            "**{choice}** - {} {}\n`{}`\n{value}",
            count,
            pluralize_vote(count),
            vote_bar(count, total_votes)
        ));
    }

    embed
        .field("Responses", response_lines.join("\n\n"), false)
        .footer(CreateEmbedFooter::new(format!("Poll ID: {}", poll.id)))
}

pub fn render_poll_buttons(poll: &Poll, responses: &[Vote]) -> Vec<CreateActionRow> {
    poll.choices
        .chunks(5)
        .enumerate()
        .map(|(chunk_index, chunk)| {
            let buttons = chunk
                .iter()
                .enumerate()
                .map(|(index, choice)| {
                    let choice_index = chunk_index * 5 + index;
                    let count = voters_for_choice(responses, choice).len();
                    CreateButton::new(format!("vote:{}:{}", poll.id, choice_index))
                        .label(format!("{choice} ({count})"))
                        .style(button_style(choice))
                })
                .collect();
            CreateActionRow::Buttons(buttons)
        })
        .collect()
}

fn voters_for_choice(responses: &[Vote], choice: &str) -> Vec<String> {
    responses
        .iter()
        .filter(|vote| vote.choice == choice)
        .map(|vote| format!("<@{}>", vote.user_id))
        .collect()
}

fn vote_bar(count: usize, total_votes: usize) -> String {
    let filled = if count == 0 || total_votes == 0 {
        0
    } else {
        (((count * 10) + (total_votes / 2)) / total_votes).clamp(1, 10)
    };
    format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled))
}

fn pluralize_vote(count: usize) -> &'static str {
    if count == 1 { "vote" } else { "votes" }
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
