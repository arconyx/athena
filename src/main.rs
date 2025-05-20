use crate::dice::roll;
use crate::quake::quake;
use poise::serenity_prelude::{self as serenity};
use std::sync::Arc;

mod dice;
mod errors;
mod quake;
mod reminders;

/// User data, which is stored and accessible in all command invocations
struct Data {
    database: Arc<reminders::ReminderDatabase>,
}

/// Helper type copied from the poise demo
/// I should probably figure out the details because we
/// use it eveywhere
type Context<'a> = poise::Context<'a, Data, errors::Error>;

/// Entry point. Setup and launch the bot.
#[tokio::main]
async fn main() {
    // Load the discord token. If it doesn't exist then panic.
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");

    // Load the database URL. If it doesn't exist return an empty string
    // which should mean we use the tokio-postgres defaults.
    let data_path = std::env::var("DATABASE_URL").unwrap_or_default();
    let database = Arc::new(
        reminders::ReminderDatabase::connect(data_path)
            .await
            .unwrap(),
    );
    // make a clone of the database for use in the closure below
    // this needs to happen here because rust spots errors if we try to `database.clone` in the framework setup
    // well there'll be an actual reason, but i'm just trusting the compiler
    let db = database.clone();

    // prepare the bot frameowrk
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            // REGISTER COMMANDS HERE
            commands: vec![quake(), reminders::remindme(), roll()],
            // register our custom error handler too
            on_error: |error| Box::pin(errors::on_error(error)),
            // and fall back to the default for everything else
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                // Setup the user data struct
                Ok(Data { database: db })
            })
        })
        .build();

    // create the bot client
    let intents = serenity::GatewayIntents::non_privileged();
    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .unwrap();

    // Spawn the reminder tasks
    // We do it now so we can pass it the bot and reuse its cache
    reminders::spawn_reminder_tasks(database.clone(), client.http.clone()).await;

    // Start the client
    client.start().await.unwrap();
}
