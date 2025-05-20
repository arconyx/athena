use std::sync::Arc;

use crate::quake::quake;
use poise::serenity_prelude::{self as serenity};

use tyche::{dice::roller::FastRand, Expr};

// User data, which is stored and accessible in all command invocations
struct Data {
    database: Arc<reminders::ReminderDatabase>,
}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

mod quake;
mod reminders;

#[poise::command(slash_command)]
async fn roll(
    ctx: Context<'_>,
    #[description = "Dice string"] message: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let expr: Expr = message.parse()?;
    let mut roller = FastRand::default();
    let roll = expr.eval(&mut roller)?;
    let description = roll.to_string();
    let total = roll.calc()?;
    ctx.say(format!("{} = {}", total, description)).await?;
    Ok(())
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);
            if let Err(e) = ctx
                .send(
                    poise::CreateReply::default().embed(
                        serenity::CreateEmbed::default()
                            .colour(serenity::Colour::RED)
                            .title("Error")
                            .description(error.to_string()),
                    ),
                )
                .await
            {
                println!("Error while reporting error: {}", e)
            }
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}

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
            on_error: |error| Box::pin(on_error(error)),
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
