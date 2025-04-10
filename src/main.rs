use std::sync::Arc;

use chrono::{DateTime, Duration, TimeDelta, Utc};
use iso8601_timestamp::Timestamp;
use poise::serenity_prelude::{
    self as serenity, futures::future, Colour, CreateEmbed, CreateMessage, UserId,
};
use serde::Deserialize;
use tokio_postgres::{connect, types::Type, Client, NoTls, Row, Statement};
use tyche::{dice::roller::FastRand, Expr};

// User data, which is stored and accessible in all command invocations
struct Data {
    database: Arc<ReminderDatabase>,
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
    select: Statement,
}
struct Reminder {
    id: i64,
    user_id: UserId,
    due_at: DateTime<Utc>,
    message: String,
}

impl Reminder {
    fn from_row(x: Row) -> Self {
        let id: i64 = x.get(0);
        let user_id_int: i64 = x.get(1);
        let user_id = UserId::from(user_id_int as u64);
        let due_at: DateTime<Utc> = x.get(2);
        let message: String = x.get(3);
        Reminder {
            id,
            user_id,
            due_at,
            message,
        }
    }
}

impl ReminderDatabase {
    async fn connect(database: String) -> Result<Self, Error> {
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
                            due_at TIMESTAMPTZ,
                            message TEXT
                        )",
                &[],
            )
            .await?;

        let (add, remove, select) = future::try_join3(
            client.prepare_typed(
                "INSERT INTO reminders (user_id, due_at, message) values ($1, $2, $3) RETURNING id",
                &[Type::INT8, Type::TIMESTAMPTZ, Type::TEXT],
            ),
            client.prepare_typed("DELETE FROM reminders WHERE id = $1", &[Type::INT8]),
            client.prepare("SELECT id, user_id, due_at, message FROM reminders"),
        )
        .await?;

        let queries = ReminderDatabase {
            client,
            add,
            remove,
            select,
        };

        Ok(queries)
    }

    async fn add_reminder(
        &self,
        user_id: UserId,
        due_at: DateTime<Utc>,
        message: String,
    ) -> Result<Reminder, Error> {
        let author_id = user_id.get() as i64;

        let id: i64 = self
            .client
            .query_one(&self.add, &[&author_id, &due_at, &message])
            .await?
            .get(0);

        Ok(Reminder {
            id,
            user_id,
            due_at,
            message,
        })
    }

    async fn remove_reminder(&self, reminder: Reminder) -> Result<(), Error> {
        self.client.execute(&self.remove, &[&reminder.id]).await?;
        Ok(())
    }

    async fn get_reminders(&self) -> Result<Vec<Row>, Error> {
        let rows = self.client.query(&self.select, &[]).await?;
        Ok(rows)
    }
}

#[derive(Debug, poise::ChoiceParameter)]
enum TimeUnitChoice {
    #[name = "seconds"]
    Seconds,
    #[name = "minutes"]
    Minutes,
    #[name = "hours"]
    Hours,
    #[name = "days"]
    Days,
    #[name = "weeks"]
    Weeks,
    #[name = "months"]
    Months,
}

fn calculate_wait(
    start: serenity::Timestamp,
    duration: i64,
    unit: TimeUnitChoice,
) -> DateTime<Utc> {
    let start_time = start.to_utc();

    let wait_duration = match unit {
        TimeUnitChoice::Seconds => Duration::seconds(duration),
        TimeUnitChoice::Minutes => Duration::minutes(duration),
        TimeUnitChoice::Hours => Duration::hours(duration),
        TimeUnitChoice::Days => Duration::days(duration),
        TimeUnitChoice::Weeks => Duration::weeks(duration),
        TimeUnitChoice::Months => Duration::days(28 * duration),
    };

    // Add the wait duration to the start time
    start_time + wait_duration
}

async fn send_reminder(bot: Arc<serenity::Http>, reminder: &Reminder) -> Result<(), Error> {
    let user = bot.get_user(reminder.user_id).await?;
    let dm_channel = user.create_dm_channel(bot.clone()).await?;

    dm_channel
        .send_message(
            bot,
            CreateMessage::default().add_embed(
                CreateEmbed::default()
                    .title("Reminder")
                    .description(reminder.message.clone())
                    .field(
                        "Scheduled For",
                        format!("<t:{}>", reminder.due_at.timestamp()),
                        false,
                    )
                    .field(
                        "Delivery Accuracy",
                        format!(
                            "{} seconds late",
                            (Utc::now() - reminder.due_at).num_seconds()
                        ),
                        false,
                    ),
            ),
        )
        .await?;

    Ok(())
}

async fn send_and_remove_reminder(
    database: Arc<ReminderDatabase>,
    bot: Arc<serenity::Http>,
    reminder: Reminder,
) {
    if let Err(e) = send_reminder(bot, &reminder).await {
        println!("Unable to send reminder: {:?}", e);
        return;
    }
    if let Err(e) = database.remove_reminder(reminder).await {
        println!("Unable to remove reminder: {:?}", e);
    }
}

async fn sleeping_reminder(
    database: Arc<ReminderDatabase>,
    bot: Arc<serenity::Http>,
    reminder: Reminder,
) {
    let delta = reminder.due_at - Utc::now();

    if delta <= TimeDelta::zero() {
        send_and_remove_reminder(database, bot, reminder).await;
        return;
    }

    let duration = match delta.to_std() {
        Ok(v) => v,
        Err(e) => {
            println!("Unable to calculate reminder instant: {}", e);
            return;
        }
    };

    tokio::time::sleep(duration).await;
    send_and_remove_reminder(database, bot, reminder).await;
}

async fn spawn_reminder_tasks(database: Arc<ReminderDatabase>, bot: Arc<serenity::Http>) {
    let Ok(rows) = database.get_reminders().await else {
        println!("Unable to get reminders");
        return;
    };

    for ele in rows {
        let reminder = Reminder::from_row(ele);
        tokio::spawn(sleeping_reminder(database.clone(), bot.clone(), reminder));
    }
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

    let database = ctx.data().database.clone();
    let author = ctx.author().id;
    let start_time = ctx.created_at();
    let end_time = calculate_wait(start_time, duration, unit);

    let reminder = database.add_reminder(author, end_time, message).await?;

    tokio::spawn(sleeping_reminder(
        database,
        ctx.serenity_context().http.clone(),
        reminder,
    ));

    ctx.say(format!("Reminder created for <t:{}>", end_time.timestamp()))
        .await?;
    Ok(())
}

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
    let database = Arc::new(ReminderDatabase::connect(data_path).await.unwrap());
    let db = database.clone();

    let intents = serenity::GatewayIntents::non_privileged();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![quake(), remindme(), roll()],
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

    spawn_reminder_tasks(database.clone(), client.http.clone()).await;

    client.start().await.unwrap();
}
