use anyhow::{Context as AnyhowContext, Result, anyhow};
use poise::serenity_prelude::{
    ComponentInteraction, Context, CreateAllowedMentions, CreateInteractionResponse,
    CreateInteractionResponseMessage, EditMessage,
};
use tracing::info;

use crate::Data;
use crate::discord::{render_poll_buttons, render_poll_embed};
use crate::models::Vote;

pub async fn handle_component(
    ctx: &Context,
    data: &Data,
    component: &ComponentInteraction,
) -> Result<()> {
    let Some(rest) = component.data.custom_id.strip_prefix("vote:") else {
        return Ok(());
    };
    let Some((poll_id, choice_index)) = rest.split_once(':') else {
        return Ok(());
    };
    let choice_index = choice_index
        .parse::<usize>()
        .context("invalid choice index")?;

    let poll = data
        .store
        .get_poll(poll_id)
        .await?
        .ok_or_else(|| anyhow!("poll {poll_id} no longer exists"))?;
    let Some(choice) = poll.choices.get(choice_index).cloned() else {
        return respond_ephemeral(ctx, component, "That choice no longer exists.").await;
    };
    let responses = data.store.poll_responses(&poll.id).await?;
    let previous_choice = user_choice(&responses, component.user.id.get()).map(str::to_string);
    let feedback = vote_feedback(previous_choice.as_deref(), &choice);

    if previous_choice.as_deref() == Some(choice.as_str()) {
        respond_ephemeral(ctx, component, &feedback).await?;
        return Ok(());
    }

    let display_name = component
        .member
        .as_ref()
        .and_then(|member| member.nick.as_deref())
        .or(component.user.global_name.as_deref())
        .unwrap_or(&component.user.name);
    data.store
        .set_response(&poll.id, component.user.id.get(), display_name, &choice)
        .await?;
    info!(
        poll_id = %poll.id,
        user_id = component.user.id.get(),
        choice = %choice,
        "recorded poll vote"
    );
    let responses = data.store.poll_responses(&poll.id).await?;
    respond_ephemeral(ctx, component, &feedback).await?;

    let mut message = component.message.as_ref().clone();
    message
        .edit(
            &ctx.http,
            EditMessage::new()
                .embed(render_poll_embed(
                    &poll,
                    &responses,
                    data.config.default_timezone,
                ))
                .components(render_poll_buttons(&poll)),
        )
        .await
        .context("failed to update poll after vote")?;
    Ok(())
}

async fn respond_ephemeral(
    ctx: &Context,
    component: &ComponentInteraction,
    content: &str,
) -> Result<()> {
    let response = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(content)
            .allowed_mentions(CreateAllowedMentions::new())
            .ephemeral(true),
    );
    component
        .create_response(&ctx.http, response)
        .await
        .context("failed to respond to component")
}

fn user_choice(responses: &[Vote], user_id: u64) -> Option<&str> {
    responses
        .iter()
        .find(|vote| vote.user_id == user_id)
        .map(|vote| vote.choice.as_str())
}

fn vote_feedback(previous_choice: Option<&str>, choice: &str) -> String {
    match previous_choice {
        Some(previous_choice) if previous_choice == choice => {
            format!("You already chose \"{}\".", plain_text(choice))
        }
        Some(previous_choice) => format!(
            "Changed your choice from \"{}\" to \"{}\".",
            plain_text(previous_choice),
            plain_text(choice)
        ),
        None => format!("You chose \"{}\".", plain_text(choice)),
    }
}

fn plain_text(value: &str) -> String {
    value
        .replace(['@', '`', '*', '_', '~', '|', '<', '>'], "")
        .replace(['\n', '\r', '\t'], " ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_vote_feedback() {
        assert_eq!(vote_feedback(None, "iva"), "You chose \"iva\".");
        assert_eq!(
            vote_feedback(Some("iva"), "le"),
            "Changed your choice from \"iva\" to \"le\"."
        );
        assert_eq!(
            vote_feedback(Some("iva"), "iva"),
            "You already chose \"iva\"."
        );
    }

    #[test]
    fn neutralizes_choice_markdown_in_feedback() {
        assert_eq!(
            vote_feedback(None, "@everyone `yes`"),
            "You chose \"everyone yes\"."
        );
    }
}
