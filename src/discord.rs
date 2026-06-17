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
        .components(render_poll_buttons(poll, responses));

    if let Some(notification) = notification {
        message = message
            .content(&notification.content)
            .allowed_mentions(notification_allowed_mentions(notification));
    }

    message
}

pub fn render_poll_embed(poll: &Poll, responses: &[Vote]) -> CreateEmbed {
    CreateEmbed::new()
        .title(&poll.title)
        .color(0x2f9eaa)
        .description(format!(
            "```md\n{}\n```",
            render_poll_panel(poll, responses)
        ))
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
    let mut lines = vec![
        format!("# EVENT: {}", clean_panel_text(&poll.title)),
        String::new(),
    ];

    if let Some(description) = &poll.description {
        lines.push(clean_panel_text(description));
        lines.push(String::new());
    }

    lines.push(format!(
        "## WHEN: {}",
        clean_panel_text(poll.when.as_deref().unwrap_or("Not specified"))
    ));
    lines.push(String::new());

    for choice_chunk in poll.choices.chunks(3) {
        lines.extend(render_choice_table(choice_chunk, responses));
        lines.push(String::new());
    }

    lines.push(format!(
        "Created by @{}  |  {}  |  ID: {}",
        clean_panel_text(
            poll.created_by_name
                .as_deref()
                .unwrap_or(&poll.created_by.to_string())
        ),
        poll.created_at.format("%B %-d, %Y"),
        poll.id
    ));

    lines.join("\n").trim_end().to_string()
}

fn render_choice_table(choices: &[String], responses: &[Vote]) -> Vec<String> {
    const COLUMN_WIDTH: usize = 22;
    let top = table_border("┌", "┬", "┐", choices.len(), COLUMN_WIDTH);
    let separator = table_border("├", "┼", "┤", choices.len(), COLUMN_WIDTH);
    let bottom = table_border("└", "┴", "┘", choices.len(), COLUMN_WIDTH);

    let mut lines = vec![top];
    lines.push(table_row(
        choices
            .iter()
            .map(|choice| align_center(&clean_panel_text(choice), COLUMN_WIDTH))
            .collect(),
    ));
    lines.push(separator.clone());
    lines.push(table_row(
        choices
            .iter()
            .map(|choice| {
                let count = responses
                    .iter()
                    .filter(|vote| vote.choice == *choice)
                    .count();
                align_center(&format!("{count} {}", pluralize_vote(count)), COLUMN_WIDTH)
            })
            .collect(),
    ));
    lines.push(separator);

    let voters = choices
        .iter()
        .map(|choice| voters_for_choice(responses, choice))
        .collect::<Vec<_>>();
    let max_voters = voters
        .iter()
        .map(|choice_voters| choice_voters.len().max(1))
        .max()
        .unwrap_or(1);

    for row_index in 0..max_voters {
        lines.push(table_row(
            voters
                .iter()
                .map(|choice_voters| {
                    let text = if choice_voters.is_empty() {
                        if row_index == 0 {
                            "Nobody yet".to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        choice_voters
                            .get(row_index)
                            .map(VoterLabel::render)
                            .unwrap_or_default()
                    };
                    align_left(&text, COLUMN_WIDTH)
                })
                .collect(),
        ));
    }
    lines.push(bottom);
    lines
}

fn table_border(
    left: &str,
    separator: &str,
    right: &str,
    columns: usize,
    column_width: usize,
) -> String {
    format!(
        "{left}{}{right}",
        std::iter::repeat_n("─".repeat(column_width), columns)
            .collect::<Vec<_>>()
            .join(separator)
    )
}

fn table_row(cells: Vec<String>) -> String {
    format!("│{}│", cells.join("│"))
}

fn align_center(value: &str, width: usize) -> String {
    let value = truncate_cell(value, width);
    let padding = width.saturating_sub(value.chars().count());
    let left = padding / 2;
    let right = padding - left;
    format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
}

fn align_left(value: &str, width: usize) -> String {
    let value = truncate_cell(value, width);
    let padding = width.saturating_sub(value.chars().count());
    format!(" {value}{}", " ".repeat(padding.saturating_sub(1)))
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

fn pluralize_vote(count: usize) -> &'static str {
    if count == 1 { "Vote" } else { "Votes" }
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
    fn renders_poll_panel_with_box_table() {
        let poll = Poll {
            id: "f531adfd".to_string(),
            title: "tal ahhar I promise".to_string(),
            description: None,
            when: Some("issa".to_string()),
            choices: vec![
                "car u tond".to_string(),
                "bajd u patata maxx".to_string(),
                "flat chested".to_string(),
            ],
            channel_id: 1,
            message_id: None,
            recurring_id: None,
            created_by: 42,
            created_by_name: Some("John".to_string()),
            created_at: Utc::now(),
        };
        let votes = vec![
            Vote {
                user_id: 1,
                display_name: Some("Philip".to_string()),
                choice: "car u tond".to_string(),
            },
            Vote {
                user_id: 2,
                display_name: Some("John".to_string()),
                choice: "car u tond".to_string(),
            },
        ];

        let panel = render_poll_panel(&poll, &votes);

        assert!(panel.contains("# EVENT: tal ahhar I promise"));
        assert!(panel.contains("## WHEN: issa"));
        assert!(panel.contains("car u tond"));
        assert!(panel.contains("2 Votes"));
        assert!(panel.contains("@Philip"));
        assert!(panel.contains("@John"));
        assert!(panel.contains("Nobody yet"));
        assert!(panel.contains("ID: f531adfd"));
    }
}
