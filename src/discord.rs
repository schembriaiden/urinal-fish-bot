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
    let mut description = Vec::new();
    if let Some(details) = &poll.description {
        description.push(details.to_string());
    }
    description.push(format!(
        "**When**\n{}\n\n**Created by** <@{}>  •  **Total votes** {}",
        poll.when.as_deref().unwrap_or("Not specified"),
        poll.created_by,
        total_votes
    ));

    let mut embed = CreateEmbed::new()
        .title(&poll.title)
        .color(0x2f9eaa)
        .description(description.join("\n\n"))
        .timestamp(poll.created_at);

    for choice in &poll.choices {
        let users = voters_for_choice(responses, choice);
        let count = users.len();
        let value = if users.is_empty() {
            "_Nobody yet_".to_string()
        } else {
            users.join("  ")
        };
        embed = embed.field(
            format!("{choice} - {} {}", count, pluralize_vote(count)),
            value,
            true,
        );
    }

    embed.footer(CreateEmbedFooter::new(format!("Poll ID: {}", poll.id)))
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
