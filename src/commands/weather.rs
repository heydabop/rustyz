use crate::{
    config,
    error::{CommandError, CommandResult},
    google,
    model::Point,
    tomorrowio,
};
use serenity::client::Context;
use serenity::model::application::interaction::application_command::{
    ApplicationCommandInteraction, CommandDataOptionValue,
};
use std::fmt::Write as _;

// Replies to msg with the weather for either the bot's location or the supplied location
// Takes a single optional argument - location as zipcode, city+state, or lat/lng in decimal form
pub async fn weather(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let args = interaction
        .data
        .options
        .get(0)
        .and_then(|o| {
            o.resolved.as_ref().map(|r| {
                if let CommandDataOptionValue::String(s) = r {
                    s
                } else {
                    ""
                }
            })
        })
        .unwrap_or("");

    let (location, location_name) = match parse_location(ctx, args).await {
        Ok((l, n)) => (l, n),
        Err(e) => {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content(e.to_string())
                })
                .await?;
            return Ok(());
        }
    };

    let api_key = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<config::TomorrowIO>().unwrap().api_key.clone()
    };
    let conditions = match tomorrowio::get_current(&location, &api_key).await {
        Ok(c) => c,
        Err(e) => return Err(e.into()),
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

    let pollen = match conditions
        .tree_index
        .max(conditions.grass_index)
        .max(conditions.weed_index)
    {
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
        r#"weather in {}
temperature | {} {}
conditions | {}
relative humidty | {} {}
wind | {} {} {}
uv index | {}
air quality index | {} {}
pollen | {}"#,
        location_name,
        conditions
            .temperature
            .map_or_else(|| "--".to_string(), |t| format!("{t:.0} \u{b0}F")),
        conditions
            .temperature_apparent
            .map_or_else(String::new, |t| format!("(feels like {t:.0} \u{b0}F)")),
        conditions_str,
        conditions
            .humidity
            .map_or_else(|| "--".to_string(), |h| format!("{h:.0}%")),
        conditions
            .dew_point
            .map_or_else(String::new, |t| format!("(dew point: {t:.0} \u{b0}F)")),
        conditions
            .wind_speed
            .map_or_else(|| "--".to_string(), |w| format!("{w:.1} mph")),
        conditions
            .wind_direction
            .map_or_else(String::new, |d| format!("from {d:.0}\u{b0}")),
        conditions
            .wind_gust
            .map_or_else(String::new, |w| format!("(gusts: {w:.1} mph)")),
        conditions
            .uv_index
            .map_or_else(|| "--".to_string(), |u| format!("{u}")),
        conditions
            .epa_index
            .map_or_else(|| "--".to_string(), |e| format!("{e}")),
        aqi_health,
        pollen
    );

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(response_msg))
        .await?;

    Ok(())
}

// Replies to msg with the hourly forecast (12h) for either the bot's location or the supplied location
// Takes a single optional argument - location as zipcode, city+state, or lat/lng in decimal form
pub async fn forecast(ctx: &Context, interaction: &ApplicationCommandInteraction) -> CommandResult {
    let mut location_args = "";
    let mut hours = 6;
    for o in &interaction.data.options {
        match &o.name[..] {
            "location" => {
                if let Some(CommandDataOptionValue::String(s)) = o.resolved.as_ref() {
                    location_args = s;
                }
            }
            "hours" => {
                if let Some(CommandDataOptionValue::Integer(h)) = o.resolved.as_ref() {
                    hours = *h;
                }
            }
            _ => {}
        }
    }

    let (location, location_name) = match parse_location(ctx, location_args).await {
        Ok((l, n)) => (l, n),
        Err(e) => {
            interaction
                .edit_original_interaction_response(&ctx.http, |response| {
                    response.content(e.to_string())
                })
                .await?;
            return Ok(());
        }
    };

    let api_key = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        data.get::<config::TomorrowIO>().unwrap().api_key.clone()
    };
    let forecast = match tomorrowio::get_hourly(&location, &api_key, hours).await {
        Ok(c) => c,
        Err(e) => return Err(e.into()),
    };

    let timezone = {
        let maps_api_key = {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            data.get::<config::Google>().unwrap().maps_api_key.clone()
        };
        match google::timezone(&location, forecast[0].start_time.timestamp(), &maps_api_key).await {
            Ok(tz) => tz,
            Err(e) => return Err(e.into()),
        }
    };

    let mut response_msg = format!(
        "weather in {}\n```   Time  | Temperature | Humidity | Dewpoint | Precipitation\n         |             |          |          | Chance\n",
        location_name
    );
    for v in forecast {
        let values = v.values;
        let time = v.start_time.with_timezone(&timezone);
        writeln!(
            response_msg,
            "{:^9}|{:^13}|{:^10}|{:^10}|{}",
            time.format("%l:%M %p"),
            values
                .temperature
                .map_or_else(|| "--".to_string(), |t| format!("{t:.0} \u{b0}F")),
            values
                .humidity
                .map_or_else(|| "--".to_string(), |t| format!("{t:.0}%")),
            values
                .dew_point
                .map_or_else(|| "--".to_string(), |t| format!("{t:.0} \u{b0}F")),
            values
                .precipitation_probability
                .map_or_else(|| " --".to_string(), |t| format!(" {t:.0}%"))
        )?;
    }
    write!(response_msg, "```")?;

    interaction
        .edit_original_interaction_response(&ctx.http, |response| response.content(response_msg))
        .await?;

    Ok(())
}

async fn parse_location(ctx: &Context, args: &str) -> Result<(Point, String), CommandError> {
    let point_regex = regex::Regex::new(r#"^(-?\d+\.?\d*)[,\s]+(-?\d+\.?\d*)$"#)?;

    if let Some(captures) = point_regex.captures(args) {
        let lat = captures.get(1).map_or("", |m| m.as_str());
        let lng = captures.get(2).map_or("", |m| m.as_str());
        let lat = match lat.parse::<f64>() {
            Ok(l) => l,
            Err(e) => return Err(e.into()),
        };
        let lng = match lng.parse::<f64>() {
            Ok(l) => l,
            Err(e) => return Err(e.into()),
        };
        let location = Point { lat, lng };
        let location_name = format!("{lat}, {lng}");
        Ok((location, location_name))
    } else if !args.is_empty() {
        let maps_api_key = {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            data.get::<config::Google>().unwrap().maps_api_key.clone()
        };
        match google::geocode(args, &maps_api_key).await {
            Ok((p, n)) => {
                let location = p;
                let location_name = n.unwrap_or_else(|| args.to_owned()).to_ascii_lowercase();
                Ok((location, location_name))
            }
            Err(e) => Err(e.into()),
        }
    } else {
        let tomorrow_io_config = {
            let data = ctx.data.read().await;
            #[allow(clippy::unwrap_used)]
            data.get::<config::TomorrowIO>().unwrap().clone()
        };
        let location = tomorrow_io_config.default_location;
        let location_name = tomorrow_io_config.default_location_name;
        Ok((location, location_name))
    }
}
