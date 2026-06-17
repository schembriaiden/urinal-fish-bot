use chrono::{DateTime, Utc};
use poise::serenity_prelude::{
    ButtonStyle, CreateActionRow, CreateButton, CreateEmbed, CreateEmbedFooter, CreateMessage,
};

use crate::models::{Poll, Vote};

pub fn render_poll_message(poll: &Poll, responses: &[Vote]) -> CreateMessage {
    CreateMessage::new()
        .embed(render_poll_embed(poll, responses))
        .components(render_poll_buttons(poll))
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
        .field("Total votes", total_votes.to_string(), true);

    if let Some(description) = &poll.description {
        embed = embed.description(description);
    }

    let mut response_lines = Vec::new();
    for choice in &poll.choices {
        let users = responses
            .iter()
            .filter(|vote| &vote.choice == choice)
            .map(|vote| format!("<@{}>", vote.user_id))
            .collect::<Vec<_>>();
        let value = if users.is_empty() {
            "Nobody yet".to_string()
        } else {
            users.join(", ")
        };
        response_lines.push(format!("**{choice}** ({})\n{value}", users.len()));
    }

    embed
        .field("Responses", response_lines.join("\n\n"), false)
        .footer(CreateEmbedFooter::new(format!("Poll ID: {}", poll.id)))
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
