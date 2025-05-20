use super::errors::Error;
use super::Context;
use crate::serenity;
use chrono::{DateTime, Duration, TimeDelta, Utc};
use poise::serenity_prelude::UserId;
use poise::serenity_prelude::{futures::future, CreateEmbed, CreateMessage};
use std::sync::Arc;
use tokio_postgres::{connect, types::Type, Client, NoTls, Row, Statement};

struct Reminder {
    pub(crate) id: i64,
    pub(crate) user_id: UserId,
    pub(crate) due_at: DateTime<Utc>,
    pub(crate) message: String,
}

impl Reminder {
    /// Convert a database row into a [`Reminder`]
    fn from_row(x: &Row) -> Self {
        let id: i64 = x.get(0);
        let user_id_int: i64 = x.get(1);

        // User ids are u64 but postgres doesn't support that so we store them as i64
        // We undo this here before it gets to the user
        #[allow(clippy::cast_sign_loss)]
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

/// Helper struct for passing around a bunch of useful stuff for working with the database.
/// Use the methods on this struct rather than directly acessing the fields.
pub(crate) struct ReminderDatabase {
    /// The database client used to interact with postgres
    client: Client,
    /// A prepared database statement that adds a reminder to the database
    add: Statement,
    /// A prepared database statement that removes a reminder from the database
    remove: Statement,
    /// A prepared database statement that fetches all reminders from the database
    select: Statement,
}

impl ReminderDatabase {
    /// Connect to the database specified by the given database string
    /// The string format is specified in the documentation for [`tokio_postgres::Config`]
    pub(crate) async fn connect(database: String) -> Result<Self, Error> {
        let (client, connection) = connect(&database, NoTls).await?;

        // The connection object performs the actual communication with the database.
        // Spawn it off to run on its own so it isn't blocking the main thread forever.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {e}");
            }
        });

        // Init table if required
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

        // Prepare the statements we'll be using. The tokio-postgres docs warn
        // > Prepared statements should be use for any query which contains user-specified data,
        // > as they provided the functionality to safely embed that data in the request.
        // > Do not form statements via string concatenation and pass them to [other] methods!
        // I believe prepared statements may also have performance benefits?
        let (add, remove, select) = future::try_join3(
            client.prepare_typed(
                "INSERT INTO reminders (user_id, due_at, message) values ($1, $2, $3) RETURNING id",
                &[Type::INT8, Type::TIMESTAMPTZ, Type::TEXT],
            ),
            client.prepare_typed("DELETE FROM reminders WHERE id = $1", &[Type::INT8]),
            client.prepare("SELECT id, user_id, due_at, message FROM reminders"),
        )
        .await?;

        // Init and return the helper
        let db_helper = ReminderDatabase {
            client,
            add,
            remove,
            select,
        };
        Ok(db_helper)
    }

    /// Add a reminder to the database
    async fn add_reminder(
        &self,
        user_id: UserId,
        due_at: DateTime<Utc>,
        message: String,
    ) -> Result<Reminder, Error> {
        // Postgres doesn't have an unsigned int 64 so we cast it to an i64
        #[allow(clippy::cast_possible_wrap)]
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

    /// Remove a reminder from the database
    async fn remove_reminder(&self, reminder: Reminder) -> Result<(), Error> {
        self.client.execute(&self.remove, &[&reminder.id]).await?;
        Ok(())
    }

    /// Get all reminders in the database.
    /// Because we purge all past reminders this should just include future reminders.
    /// However this is not guaranteed.
    async fn get_reminders(&self) -> Result<Vec<Row>, Error> {
        let rows = self.client.query(&self.select, &[]).await?;
        Ok(rows)
    }
}

/// Helper enum for the available time periods
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
    /// Actually 28 days
    #[name = "months"]
    Months,
}

/// Calculate when a reminder is due from the start time and duration.
/// The quantity and unit of the duration are passed as seperate parameters.
fn calculate_wait(
    start: serenity::Timestamp,
    duration: i64,
    unit: &TimeUnitChoice,
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

/// Deliver a reminder to a user in their direct messages
async fn send_reminder(bot: Arc<serenity::Http>, reminder: &Reminder) -> Result<(), Error> {
    // Get the user's DMs
    let user = bot.get_user(reminder.user_id).await?;
    let dm_channel = user.create_dm_channel(bot.clone()).await?;

    // Prepare and send the message
    let message = CreateMessage::default().add_embed(
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
    );
    dm_channel.send_message(bot, message).await?;

    Ok(())
}

/// Send a reminder to the user.
/// If successful, remove it from the database.
/// If not, log an error and leave the reminder in the database
/// so it can be retired later.
async fn send_and_remove_reminder(
    database: Arc<ReminderDatabase>,
    bot: Arc<serenity::Http>,
    reminder: Reminder,
) {
    if let Err(e) = send_reminder(bot, &reminder).await {
        println!("Unable to send reminder: {e:?}");
        return;
    }
    if let Err(e) = database.remove_reminder(reminder).await {
        println!("Unable to remove reminder: {e:?}");
    }
}

/// Sleep until a reminder is due, then deliver it and remove it from the database
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
            println!("Unable to calculate reminder instant: {e}");
            return;
        }
    };

    tokio::time::sleep(duration).await;
    send_and_remove_reminder(database, bot, reminder).await;
}

/// For every active reminder spawn a task that will sleep until it is
/// due then deliver it
pub(crate) async fn spawn_reminder_tasks(
    database: Arc<ReminderDatabase>,
    bot: Arc<serenity::Http>,
) {
    let Ok(rows) = database.get_reminders().await else {
        println!("Unable to get reminders");
        return;
    };

    for ele in rows {
        let reminder = Reminder::from_row(&ele);
        tokio::spawn(sleeping_reminder(database.clone(), bot.clone(), reminder));
    }
}

/// Create a reminder about something
#[poise::command(slash_command, subcommands("remindin"))]
pub(crate) async fn remindme(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Please use a subcommand").await?;
    Ok(())
}

// Space is open for a `/remindme at` command

/// Remind me in...
#[poise::command(slash_command, rename = "in")]
pub(crate) async fn remindin(
    ctx: Context<'_>,
    #[description = "Time till reminder"]
    #[min = 1]
    #[max = 10000]
    duration: i64,
    #[description = "Time units"] unit: TimeUnitChoice,
    #[description = "Reminder message"] message: String,
) -> Result<(), Error> {
    // yes discord, we're working on it
    // don't time us out yet
    ctx.defer().await?;

    // write the reminder to the database
    let database = ctx.data().database.clone();
    let author = ctx.author().id;
    let start_time = ctx.created_at();
    let end_time = calculate_wait(start_time, duration, &unit);
    let reminder = database.add_reminder(author, end_time, message).await?;

    // spawn a task to deliver the reminder
    tokio::spawn(sleeping_reminder(
        database,
        ctx.serenity_context().http.clone(),
        reminder,
    ));

    // tell the user that everything is hunky-dory
    ctx.say(format!("Reminder created for <t:{}>", end_time.timestamp()))
        .await?;
    Ok(())
}
