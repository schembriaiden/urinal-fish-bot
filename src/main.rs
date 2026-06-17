mod choices;
mod commands;
mod components;
mod config;
mod discord;
mod easter_egg;
mod models;
mod recurrence;
mod scheduler;
mod storage;
mod validation;

use std::sync::Arc;

use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude as serenity;
use serenity::{EventHandler, Interaction, async_trait};
use tracing::{error, info};

use crate::config::Config;
use crate::storage::Store;

type Error = anyhow::Error;
type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(Clone)]
pub struct Data {
    pub config: Config,
    pub store: Store,
}

struct ComponentHandler {
    data: Data,
}

#[async_trait]
impl EventHandler for ComponentHandler {
    async fn interaction_create(&self, ctx: serenity::Context, interaction: Interaction) {
        let result = match interaction {
            Interaction::Component(component) => {
                components::handle_component(&ctx, &self.data, &component).await
            }
            _ => Ok(()),
        };

        if let Err(err) = result {
            error!("component event failed: {err:?}");
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "urinal_fish_bot=info,info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let token = config.token.clone();
    let store = Store::open(&config.database_path).await?;
    let data = Data { config, store };
    let setup_data = data.clone();
    let component_data = data.clone();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::commands(),
            ..Default::default()
        })
        .setup(move |ctx, ready, framework| {
            let data = setup_data.clone();
            Box::pin(async move {
                let commands =
                    poise::builtins::create_application_commands(&framework.options().commands);
                data.config
                    .guild_id
                    .set_commands(&ctx.http, commands)
                    .await
                    .context("failed to register guild slash commands")?;
                info!("{} is connected", ready.user.name);

                tokio::spawn(scheduler::run(Arc::new(data.clone()), ctx.http.clone()));
                Ok(data)
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged();
    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .event_handler(ComponentHandler {
            data: component_data,
        })
        .await
        .context("failed to create Discord client")?;

    client.start().await.context("Discord client stopped")?;
    Ok(())
}
