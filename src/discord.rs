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
    let mut embed = CreateEmbed::new().title(&poll.title);

    if let Some(description) = &poll.description {
        embed = embed.description(description);
    }
    if let Some(when) = &poll.when {
        embed = embed.field("When", when, false);
    }

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
        embed = embed.field(format!("{choice} ({})", users.len()), value, false);
    }

    embed.footer(CreateEmbedFooter::new(format!("Poll ID: {}", poll.id)))
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
                        .style(ButtonStyle::Primary)
                })
                .collect();
            CreateActionRow::Buttons(buttons)
        })
        .collect()
}

pub fn format_discord_time(time: DateTime<Utc>) -> String {
    format!("<t:{}:F>", time.timestamp())
}
