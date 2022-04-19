use crate::{config, google, model::Point, tomorrowio};
use serenity::client::Context;
use serenity::framework::standard::{macros::command, Args, CommandError, CommandResult};
use serenity::model::channel::Message;

// Replies to msg with the weather for either the bot's location or the supplied location
// Takes a single optional argument - location as zipcode, city+state, or lat/lng in decimal form
#[command]
pub async fn weather(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = args.rest();
    let mut location: Option<Point> = None;
    let point_regex = regex::Regex::new(r#"^(-?\d+\.?\d*)[,\s]+(-?\d+\.?\d*)$"#).unwrap();

    if let Some(captures) = point_regex.captures(args) {
        let lat = captures.get(1).map_or("", |m| m.as_str());
        let lng = captures.get(2).map_or("", |m| m.as_str());
        let lat = match lat.parse::<f64>() {
            Ok(l) => l,
            Err(e) => {
                crate::util::record_say(ctx, msg, e.to_string()).await?;
                return Ok(());
            }
        };
        let lng = match lng.parse::<f64>() {
            Ok(l) => l,
            Err(e) => {
                crate::util::record_say(ctx, msg, e.to_string()).await?;
                return Ok(());
            }
        };
        location = Some(Point { lat, lng });
    } else if !args.is_empty() {
        let maps_api_key = {
            let data = ctx.data.read().await;
            data.get::<config::Google>().unwrap().maps_api_key.clone()
        };
        match google::geocode(args, &maps_api_key).await {
            Ok(p) => location = Some(p),
            Err(e) => match e {
                google::Error::Reqwest(e) => return Err(CommandError::from(e)),
                e => {
                    crate::util::record_say(ctx, msg, e).await?;
                    return Ok(());
                }
            },
        }
    }

    let tomorrow_io_config = {
        let data = ctx.data.read().await;
        data.get::<config::TomorrowIO>().unwrap().clone()
    };
    let location = if let Some(location) = location {
        format!("{},{}", location.lat, location.lng)
    } else {
        tomorrow_io_config.default_location_id.clone()
    };
    let conditions = match tomorrowio::get_current(&location, &tomorrow_io_config.api_key).await {
        Ok(c) => c,
        Err(e) => return Err(CommandError::from(e)),
    };

    let aqi_health = match conditions.epa_index {
        Some(a) => {
            if a < 51 {
                "(Good)"
            } else if a < 101 {
                "(Moderate)"
            } else if a < 151 {
                "(Unhealthy for sensitive groups)"
            } else if a < 201 {
                "(Unhealthy)"
            } else if a < 301 {
                "(Very Unhealthy)"
            } else {
                "(Hazardous)"
            }
        }
        None => "",
    };

    let conditions_str = match conditions.weather_code {
        Some(c) => match c {
            1000 => "clear",
            1001 => "cloudy",
            1100 => "mostly clear",
            1101 => "partly cloudy",
            1102 => "mostly cloudy",
            2000 => "fog",
            2100 => "light fog",
            3000 => "light wind",
            3001 => "wind",
            3002 => "strong wind",
            4000 => "drizzle",
            4001 => "rain",
            4200 => "light rain",
            4201 => "heavy rain",
            5000 => "snow",
            5001 => "flurries",
            5100 => "light snow",
            5101 => "heavy snow",
            6000 => "freezing drizzle",
            6001 => "freezing rain",
            6200 => "light freezing rain",
            6201 => "heavy freezing rain",
            7000 => "ice pellets",
            7101 => "heavy ice pellets",
            7102 => "light ice pellets",
            8000 => "thunderstorm",
            _ => "unknown",
        },
        None => "unknown",
    };

    let pollen = match conditions.tree_index.max(conditions.grass_index).max(conditions.weed_index) {
        Some(t) => match t {
            0 => "none",
            1 => "very low",
            2 => "low",
            3 => "medium",
            4 => "high",
            5 => "very high",
            _ => "unknown",
        },
        None => "unknown",
    };

    let response_msg = format!(
        r#"temperature | {} {}
conditions | {}
relative humidty | {} {}
wind | {} {} {}
uv index | {}
air quality index | {} {}
pollen | {}"#,
        conditions
            .temperature
            .map_or_else(|| "--".to_string(), |t| format!("{:.0} \u{b0}F", t)),
        conditions.temperature_apparent.map_or_else(
            || "".to_string(),
            |t| format!("(feels like {:.0} \u{b0}F)", t)
        ),
        conditions_str,
        conditions
            .humidity
            .map_or_else(|| "--".to_string(), |h| format!("{:.0}%", h)),
        conditions.dewpoint.map_or_else(
            || "".to_string(),
            |t| format!("(dew point: {:.0} \u{b0}F)", t)
        ),
        conditions
            .wind_speed
            .map_or_else(|| "--".to_string(), |w| format!("{:.1} mph", w)),
        conditions
            .wind_direction
            .map_or_else(|| "".to_string(), |d| format!("from {:.0}\u{b0}", d)),
        conditions
            .wind_gust
            .map_or_else(|| "".to_string(), |w| format!("(gusts: {:.1} mph)", w)),
        conditions
            .uv_index
            .map_or_else(|| "--".to_string(), |u| format!("{}", u)),
        conditions
            .epa_index
            .map_or_else(|| "--".to_string(), |e| format!("{}", e)),
        aqi_health,
        pollen
    );

    crate::util::record_say(ctx, msg, response_msg).await?;

    Ok(())
}
