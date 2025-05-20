use super::Context;
use super::Error;
use tyche::dice::roller::FastRand;
use tyche::Expr;

#[poise::command(slash_command)]
pub(crate) async fn roll(
    ctx: Context<'_>,
    #[description = "Dice string"] message: String,
) -> Result<(), Error> {
    ctx.defer().await?;
    let expr: Expr = message.parse()?;
    let mut roller = FastRand::default();
    let roll = expr.eval(&mut roller)?;
    let description = roll.to_string();
    let total = roll.calc()?;
    ctx.say(format!("{total} = {description}")).await?;
    Ok(())
}
