use super::errors::Error;
use super::Context;
use crate::serenity;
use iso8601_timestamp::Timestamp;
use poise::serenity_prelude::Colour;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct QuakeProperties {
    #[serde(rename = "publicID")]
    pub(crate) public_id: String,
    pub(crate) time: Timestamp,
    pub(crate) depth: f64,
    pub(crate) locality: String,
    pub(crate) magnitude: f64,
    pub(crate) mmi: i8,
    pub(crate) quality: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Quake {
    pub(crate) properties: QuakeProperties,
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
                i8::MIN..=7 => format!("Most recent quake with MMI >= {mmi}"),
                8..=i8::MAX => format!("Well, fuck. Most recent quake with MMI >= {mmi}"),
            })
            .field("Magnitude", format!("{:.3}", properties.magnitude), true)
            .field("MMI", properties.mmi.to_string(), true)
            .field("Depth", format!("{:.3} km", properties.depth), true)
            .field("Time", format!("<t:{timestamp}:R>"), true)
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
    pub(crate) features: Vec<Quake>,
}

async fn get_quake(mmi: i8) -> Result<Quake, Error> {
    let url = format!("https://api.geonet.org.nz//quake?MMI={mmi}");
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
pub(crate) async fn quake(
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
