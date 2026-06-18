use anyhow::{Context as AnyhowContext, Result, anyhow};
use poise::serenity_prelude::{
    ComponentInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use tracing::info;

use crate::Data;
use crate::discord::{render_poll_buttons, render_poll_embed};

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
    let response = CreateInteractionResponse::UpdateMessage(
        CreateInteractionResponseMessage::new()
            .embed(render_poll_embed(
                &poll,
                &responses,
                data.config.default_timezone,
            ))
            .components(render_poll_buttons(&poll)),
    );

    component
        .create_response(&ctx.http, response)
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
            .ephemeral(true),
    );
    component
        .create_response(&ctx.http, response)
        .await
        .context("failed to respond to component")
}
