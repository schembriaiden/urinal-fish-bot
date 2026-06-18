use chrono::{DateTime, Utc};
use poise::serenity_prelude::{
    ButtonStyle, CreateActionRow, CreateAllowedMentions, CreateButton, CreateEmbed, CreateMessage,
    RoleId, UserId,
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
    CreateEmbed::new().color(0x2f9eaa).description(format!(
        "```text\n{}\n```",
        render_poll_panel(poll, responses)
    ))
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

fn voters_for_choice(responses: &[Vote], choice: &str) -> Vec<VoterLabel> {
    responses
        .iter()
        .filter(|vote| vote.choice == choice)
        .map(|vote| VoterLabel {
            user_id: vote.user_id,
            display_name: vote.display_name.clone(),
        })
        .collect()
}

fn render_poll_panel(poll: &Poll, responses: &[Vote]) -> String {
    const PANEL_WIDTH: usize = 58;
    const COLUMN_WIDTH: usize = 18;
    let mut lines = vec![top_border(PANEL_WIDTH)];

    lines.extend(panel_wrapped_line(
        &format!("📅 {}", clean_panel_text(&poll.title)),
        PANEL_WIDTH,
    ));
    if let Some(description) = &poll.description {
        lines.extend(panel_wrapped_line(
            &clean_panel_text(description),
            PANEL_WIDTH,
        ));
    }
    lines.push(panel_blank_line(PANEL_WIDTH));

    lines.extend(panel_columns(
        &["🗓 When", "📍 Where", "👤 Creator"],
        COLUMN_WIDTH,
        PANEL_WIDTH,
    ));
    lines.extend(panel_columns(
        &[
            clean_panel_text(poll.when.as_deref().unwrap_or("Not specified")),
            clean_panel_text(poll.location.as_deref().unwrap_or("Not specified")),
            format!(
                "@{}",
                clean_panel_text(
                    poll.created_by_name
                        .as_deref()
                        .unwrap_or(&poll.created_by.to_string())
                )
            ),
        ],
        COLUMN_WIDTH,
        PANEL_WIDTH,
    ));
    lines.push(panel_blank_line(PANEL_WIDTH));

    for choice_chunk in poll.choices.chunks(3) {
        lines.extend(render_choice_columns(
            choice_chunk,
            responses,
            COLUMN_WIDTH,
            PANEL_WIDTH,
        ));
        lines.push(panel_blank_line(PANEL_WIDTH));
    }

    lines.extend(panel_wrapped_line(
        &format!(
            "Event ID: {} • Created on: {}",
            poll.id,
            poll.created_at.format("%-d %B %Y at %H:%M")
        ),
        PANEL_WIDTH,
    ));
    lines.push(bottom_border(PANEL_WIDTH));
    lines.join("\n")
}

fn render_choice_columns(
    choices: &[String],
    responses: &[Vote],
    column_width: usize,
    panel_width: usize,
) -> Vec<String> {
    let headers = choices
        .iter()
        .map(|choice| {
            let count = responses
                .iter()
                .filter(|vote| vote.choice == *choice)
                .count();
            format!("{} - {count}", clean_panel_text(choice))
        })
        .collect::<Vec<_>>();
    let voters = choices
        .iter()
        .map(|choice| {
            let labels = voters_for_choice(responses, choice)
                .into_iter()
                .map(|voter| voter.render())
                .collect::<Vec<_>>();
            if labels.is_empty() {
                vec!["No one yet".to_string()]
            } else {
                labels
            }
        })
        .collect::<Vec<_>>();
    let max_voters = voters.iter().map(Vec::len).max().unwrap_or(1);

    let mut lines = panel_columns(&headers, column_width, panel_width);
    for row_index in 0..max_voters {
        let row = voters
            .iter()
            .map(|choice_voters| choice_voters.get(row_index).cloned().unwrap_or_default())
            .collect::<Vec<_>>();
        lines.extend(panel_columns(&row, column_width, panel_width));
    }
    lines
}

fn top_border(width: usize) -> String {
    format!("╭{}╮", "─".repeat(width))
}

fn bottom_border(width: usize) -> String {
    format!("╰{}╯", "─".repeat(width))
}

fn panel_blank_line(width: usize) -> String {
    panel_line("", width)
}

fn panel_wrapped_line(value: &str, panel_width: usize) -> Vec<String> {
    wrap_text(value, panel_width)
        .into_iter()
        .map(|line| panel_line(&line, panel_width))
        .collect()
}

fn panel_columns(
    values: &[impl AsRef<str>],
    column_width: usize,
    panel_width: usize,
) -> Vec<String> {
    let wrapped = values
        .iter()
        .map(|value| wrap_text(value.as_ref(), column_width))
        .collect::<Vec<_>>();
    let max_rows = wrapped.iter().map(Vec::len).max().unwrap_or(1);
    let mut lines = Vec::with_capacity(max_rows);

    for row_index in 0..max_rows {
        let content = wrapped
            .iter()
            .map(|lines| {
                let value = lines.get(row_index).map(String::as_str).unwrap_or("");
                pad_right(value, column_width)
            })
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(panel_line(&content, panel_width));
    }

    lines
}

fn panel_line(content: &str, width: usize) -> String {
    format!("│{}│", pad_right(content, width))
}

fn wrap_text(value: &str, width: usize) -> Vec<String> {
    let value = clean_panel_text(value);
    if value.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in value.split_whitespace() {
        let separator = usize::from(!current.is_empty());
        if current.chars().count() + separator + word.chars().count() <= width {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            continue;
        }
        if !current.is_empty() {
            lines.push(current);
            current = String::new();
        }
        if word.chars().count() <= width {
            current.push_str(word);
        } else {
            lines.extend(split_long_word(word, width));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn split_long_word(value: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if current.chars().count() == width {
            lines.push(current);
            current = String::new();
        }
        current.push(character);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn pad_right(value: &str, width: usize) -> String {
    let value = truncate_cell(value, width);
    let padding = width.saturating_sub(value.chars().count());
    format!("{value}{}", " ".repeat(padding))
}

fn truncate_cell(value: &str, width: usize) -> String {
    let value = clean_panel_text(value);
    if value.chars().count() <= width {
        return value;
    }
    value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>()
        + "…"
}

fn clean_panel_text(value: &str) -> String {
    value
        .replace('`', "'")
        .replace(['\n', '\r', '\t'], " ")
        .trim()
        .to_string()
}

struct VoterLabel {
    user_id: u64,
    display_name: Option<String>,
}

impl VoterLabel {
    fn render(&self) -> String {
        format!(
            "@{}",
            clean_panel_text(
                self.display_name
                    .as_deref()
                    .unwrap_or(&self.user_id.to_string())
            )
        )
    }
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
    fn renders_poll_panel_with_final_layout() {
        let poll = Poll {
            id: "f531adfd".to_string(),
            title: "Tal-Aħħar".to_string(),
            description: Some("issa".to_string()),
            when: Some("Today at 22:30".to_string()),
            location: Some("Valletta".to_string()),
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

        let panel = render_poll_panel(&poll, &votes);

        assert!(panel.contains("📅 Tal-Aħħar"));
        assert!(panel.contains("🗓 When"));
        assert!(panel.contains("📍 Where"));
        assert!(panel.contains("👤 Creator"));
        assert!(panel.contains("caru tond - 2"));
        assert!(panel.contains("bajd u patata - 1"));
        assert!(panel.contains("flat - 0"));
        assert!(panel.contains("@FriendOne"));
        assert!(panel.contains("@EventCreator"));
        assert!(panel.contains("@FriendTwo"));
        assert!(panel.contains("No one yet"));
        assert!(panel.contains("Event ID: f531adfd"));
    }
}
