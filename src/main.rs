use poise::serenity_prelude::{self as serenity};
use serde::Deserialize;

struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(Deserialize)]
struct QuakeProperties {
    #[serde(rename = "publicID")]
    public_id: String,
    time: String,
    depth: f64,
    locality: String,
    magnitude: f64,
    mmi: i8,
    quality: String,
}

#[derive(Deserialize)]
struct Quake {
    properties: QuakeProperties,
}

#[derive(Deserialize)]
struct QuakeList {
    // r#type: String,
    features: Vec<Quake>,
}

async fn get_quake(mmi: i8) -> Result<Quake, Error> {
    let url = format!("https://api.geonet.org.nz//quake?MMI={}", mmi);
    let client = reqwest::Client::new();

    let mut quakes = client
        .get(url)
        .header("Accept", "application/vnd.geo+json;version=2")
        .send()
        .await?
        .json::<QuakeList>()
        .await?
        .features;

    quakes.sort_by(|a, b| a.properties.time.cmp(&b.properties.time));
    quakes.pop().ok_or("No quakes found".into())
}

/// Displays the most recent quake
#[poise::command(slash_command)]
async fn quake(
    ctx: Context<'_>,
    #[description = "Minimum intensity: 0-8"]
    #[min = 0]
    // negative -1 is the true minimum imposed by the API but then rust-analyzer complains and I can't find the single-line offswitch
    #[max = 8]
    minimum_mmi: Option<i8>,
) -> Result<(), Error> {
    let mmi = minimum_mmi.unwrap_or(3);
    let quake = get_quake(mmi).await?;

    ctx.say(quake.properties.public_id).await?;
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
                        serenity::CreateEmbed::new()
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
    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![quake()],
            on_error: |error| Box::pin(on_error(error)),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}
