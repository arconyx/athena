use super::errors::Error;
use super::Context;
use crate::serenity;
use iso8601_timestamp::Timestamp;
use poise::serenity_prelude::Colour;
use serde::Deserialize;

/// This structure corresponds to the `properties` compound in
/// the data structure for a quake in the geonet api.
/// Used to deserialize JSON quake data with Serde
#[derive(Debug, Clone, Deserialize)]
struct QuakeProperties {
    #[serde(rename = "publicID")] // rename to match rust style conventions
    pub(crate) public_id: String,
    pub(crate) time: Timestamp,
    pub(crate) depth: f64,
    pub(crate) locality: String,
    pub(crate) magnitude: f64,
    pub(crate) mmi: i8,
    pub(crate) quality: String,
}

/// A quake, as repesented by geonet
#[derive(Debug, Clone, Deserialize)]
struct Quake {
    // We ignore the geometry compound and only keep the `properties`
    pub(crate) properties: QuakeProperties,
}

// Define some methods for the Quake struct
impl Quake {
    /// Convert a [`Quake`] to a [`serenity::CreateEmbed`],
    /// a builder for an embed in a Discord message
    fn create_embed(&self, mmi: i8) -> serenity::CreateEmbed {
        // Prepare some data
        let properties = &self.properties;
        let timestamp = properties
            .time
            .duration_since(Timestamp::UNIX_EPOCH)
            .whole_seconds();

        // Create the embed
        serenity::CreateEmbed::default()
            .url(format!(
                "https://www.geonet.org.nz/earthquake/{}",
                properties.public_id
            ))
            .title(format!("Quake ID {}", properties.public_id))
            .description(match mmi {
                i8::MIN..=7 => format!("Most recent quake with MMI >= {mmi}"),
                // Special handling for a very bad day
                8..=i8::MAX => format!("Well, fuck. Most recent quake with MMI >= {mmi}"),
            })
            .field("Magnitude", format!("{:.3}", properties.magnitude), true)
            .field("MMI", properties.mmi.to_string(), true)
            .field("Depth", format!("{:.3} km", properties.depth), true)
            .field("Time", format!("<t:{timestamp}:R>"), true)
            .field("Quality", properties.quality.to_string(), true)
            .field("Location", &properties.locality, true)
            // Colour code the embed to match the severity
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

/// A structure for deserializing geonet's quake list
#[derive(Debug, Clone, Deserialize)]
struct QuakeList {
    // The only information we care about is the list of quakes in the `feature` key
    pub(crate) features: Vec<Quake>,
}

/// Poll geonet for all quakes at or above the given API and return the
/// most recent. If no such quake exists then return an error.
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

/// Displays the most recent quake >= specified intensity (MMI)
#[poise::command(slash_command)]
pub(crate) async fn quake(
    ctx: Context<'_>,
    #[description = "Minimum intensity: 0-8"]
    // negative -1 is the true minimum imposed by the API but then rust-analyzer complains and I can't find the single-line offswitch
    #[min = 0]
    #[max = 8]
    minimum_mmi: Option<i8>,
) -> Result<(), Error> {
    // let the server know we're thinking about it
    ctx.defer().await?;

    // fetch the quake from the api
    let mmi = minimum_mmi.unwrap_or(3);
    let quake = get_quake(mmi).await?;

    // return the response
    let embed = quake.create_embed(mmi);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}
