use super::Data;
use crate::{serenity, Context};
use poise::FrameworkError;

/// Standard type for errors
pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;

/// Custom error handler.
/// We implement custom handling for some errors and forward the rest onto the default handler.
/// Currently we have custom handling for [`FrameworkError::Setup`] and [`FrameworkError::Command`].
pub(crate) async fn on_error(error: FrameworkError<'_, Data, Error>) {
    match error {
        FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {error:?}"),
        FrameworkError::Command { error, ctx, .. } => send_error_message(ctx, error).await,
        error => delegate_to_default_handler(error).await,
    }
}

/// Send an error message to Discord in response to a com,amd
async fn send_error_message(ctx: Context<'_>, error: Error) {
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
        println!("Error while reporting error: {e}");
    }
}

// Delegate to poise and print an error to the lgos if that fails
async fn delegate_to_default_handler(error: FrameworkError<'_, Data, Error>) {
    if let Err(e) = poise::builtins::on_error(error).await {
        println!("Error while handling error: {e}");
    }
}
