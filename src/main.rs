use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use iso8601_timestamp::Timestamp;
use poise::serenity_prelude::{self as serenity, futures::future, Colour, UserId};
use serde::Deserialize;
use tokio_postgres::{connect, types::Type, Client, NoTls, Statement};

// User data, which is stored and accessible in all command invocations
struct Data {
    database: ReminderDatabase,
}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(Debug, Clone, Deserialize)]
struct QuakeProperties {
    #[serde(rename = "publicID")]
    public_id: String,
    time: Timestamp,
    depth: f64,
    locality: String,
    magnitude: f64,
    mmi: i8,
    quality: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Quake {
    properties: QuakeProperties,
}

impl Quake {
    fn create_embed(&self, mmi: i8) -> serenity::CreateEmbed {
        let properties = &self.properties;
        let timestamp = properties
            .time
            .duration_since(Timestamp::UNIX_EPOCH)
            .whole_seconds();

        serenity::CreateEmbed::default()
            .url(format!(
                "https://www.geonet.org.nz/earthquake/{}",
                properties.public_id
            ))
            .title(format!("Quake ID {}", properties.public_id))
            .description(match mmi {
                i8::MIN..=7 => format!("Most recent quake with MMI >= {}", mmi),
                8..=i8::MAX => format!("Well, fuck. Most recent quake with MMI >= {}", mmi),
            })
            .field("Magnitude", format!("{:.3}", properties.magnitude), true)
            .field("MMI", properties.mmi.to_string(), true)
            .field("Depth", format!("{:.3} km", properties.depth), true)
            .field("Time", format!("<t:{}:R>", timestamp), true)
            .field("Quality", properties.quality.to_string(), true)
            .field("Location", &properties.locality, true)
            .color(match mmi {
                i8::MIN..=0 => Colour::LIGHT_GREY,
                1 => Colour::from_rgb(255, 255, 238),
                2 => Colour::from_rgb(255, 236, 210),
                3 => Colour::from_rgb(255, 207, 182),
                4 => Colour::from_rgb(255, 179, 155),
                5 => Colour::from_rgb(255, 151, 129),
                6 => Colour::from_rgb(244, 124, 104),
                7 => Colour::from_rgb(213, 98, 79),
                8..=i8::MAX => Colour::from_rgb(153, 45, 34),
            })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct QuakeList {
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
    quakes
        .pop()
        .ok_or("No quakes found with the required intensity".into())
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
    ctx.defer().await?;

    let mmi = minimum_mmi.unwrap_or(3);
    let quake = get_quake(mmi).await?;

    let embed = quake.create_embed(mmi);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

struct ReminderDatabase {
    client: Client,
    add: Statement,
    remove: Statement,
}

async fn connect_to_database(database: String) -> Result<ReminderDatabase, Error> {
    let (client, connection) = connect(&database, NoTls).await?;

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    client
        .execute(
            "CREATE TABLE IF NOT EXISTS reminders (
                        id BIGSERIAL PRIMARY KEY,
                        user_id BIGINT,
                        time TIMESTAMP,
                        message TEXT
                    )",
            &[],
        )
        .await?;

    let (add, remove) = future::try_join(
        client.prepare_typed(
            "INSERT INTO reminders (user_id, time, message) values ($1, $2, $3)",
            &[Type::INT8, Type::TIMESTAMP, Type::TEXT],
        ),
        client.prepare_typed("DELETE FROM reminders WHERE id = $1", &[Type::INT8]),
    )
    .await?;

    let queries = ReminderDatabase {
        client,
        add,
        remove,
    };

    Ok(queries)
}

#[derive(Debug, poise::ChoiceParameter)]
enum TimeUnitChoice {
    #[name = "seconds"]
    SECONDS,
    #[name = "minutes"]
    MINUTES,
    #[name = "hours"]
    HOURS,
    #[name = "days"]
    DAYS,
    #[name = "weeks"]
    WEEKS,
    #[name = "months"]
    MONTHS,
}

fn calculate_wait(
    start: serenity::Timestamp,
    duration: i64,
    unit: TimeUnitChoice,
) -> DateTime<Utc> {
    let start_time = start.to_utc();

    let wait_duration = match unit {
        TimeUnitChoice::SECONDS => Duration::seconds(duration),
        TimeUnitChoice::MINUTES => Duration::minutes(duration),
        TimeUnitChoice::HOURS => Duration::hours(duration),
        TimeUnitChoice::DAYS => Duration::days(duration),
        TimeUnitChoice::WEEKS => Duration::weeks(duration),
        TimeUnitChoice::MONTHS => Duration::days(28 * duration),
    };

    // Add the wait duration to the start time
    start_time + wait_duration
}

async fn add_reminder(
    database: &ReminderDatabase,
    author: &UserId,
    due_at: &NaiveDateTime,
    message: &String,
) -> Result<(), Error> {
    let author_id = author.get() as i64;

    database
        .client
        .execute(&database.add, &[&author_id, due_at, message])
        .await?;

    Ok(())
}

/// Create a reminder about something
#[poise::command(slash_command, subcommands("remindin"))]
async fn remindme(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Please use a subcommand").await?;
    Ok(())
}

#[poise::command(slash_command, rename = "in")]
async fn remindin(
    ctx: Context<'_>,
    #[description = "Time till reminder"]
    #[min = 1]
    #[max = 10000]
    duration: i64,
    #[description = "Time units"] unit: TimeUnitChoice,
    #[description = "Reminder message"] message: String,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let database = &ctx.data().database;
    let author = ctx.author().id;
    let start_time = ctx.created_at();
    let end_time = calculate_wait(start_time, duration, unit);

    add_reminder(database, &author, &end_time.naive_utc(), &message).await?;
    ctx.say(format!("Reminder created for <t:{}>", end_time.timestamp()))
        .await?;
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

    let intents = serenity::GatewayIntents::non_privileged();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![quake(), remindme()],
            on_error: |error| Box::pin(on_error(error)),
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                let data_path = std::env::var("DATABASE_URL").unwrap_or_default();
                let database = connect_to_database(data_path).await?;

                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data { database })
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}
