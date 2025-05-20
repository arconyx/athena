use crate::dice::roll;
use crate::quake::quake;
use poise::serenity_prelude::{self as serenity};
use std::sync::Arc;

mod dice;
mod quake;
mod reminders;

/// User data, which is stored and accessible in all command invocations
struct Data {
    database: Arc<reminders::ReminderDatabase>,
}

type Context<'a> = poise::Context<'a, Data, errors::Error>;

mod errors;

#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");

    let data_path = std::env::var("DATABASE_URL").unwrap_or_default();
    let database = Arc::new(
        reminders::ReminderDatabase::connect(data_path)
            .await
            .unwrap(),
    );
    let db = database.clone();

    let intents = serenity::GatewayIntents::non_privileged();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![quake(), reminders::remindme(), roll()],
            on_error: |error| Box::pin(errors::on_error(error)),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data { database: db })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .unwrap();

    reminders::spawn_reminder_tasks(database.clone(), client.http.clone()).await;

    client.start().await.unwrap();
}
