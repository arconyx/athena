use super::errors::Error;
use super::Context;
use tyche::dice::roller::FastRand;
use tyche::Expr;

/// Roll some dice based on a tyche dice expression
#[poise::command(slash_command)]
pub(crate) async fn roll(
    ctx: Context<'_>,
    #[description = "Tyche compatible dice string"] dice: String,
) -> Result<(), Error> {
    // let the server know we're working on it
    ctx.defer().await?;

    // parse expression and roll dice
    let expr: Expr = dice.parse()?;
    // creating a new roller every time is maybe a bit wasteful but it avoids any scope or lifetime issues
    let mut roller = FastRand::default();
    let roll = expr.eval(&mut roller)?;
    let description = roll.to_string();
    let total = roll.calc()?;

    // respond to user
    ctx.say(format!("{total} = {description}")).await?;
    Ok(())
}
