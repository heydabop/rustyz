use crate::{
    airnow, config,
    error::{CommandError, CommandResult},
    google,
    model::Point,
    tomorrowio,
};
use serenity::all::{CommandDataOptionValue, CommandInteraction};
use serenity::builder::EditInteractionResponse;
use serenity::client::Context;
use std::fmt::Write as _;

// Replies to msg with the weather for either the bot's location or the supplied location
// Takes a single optional argument - location as zipcode, city+state, or lat/lng in decimal form
pub async fn weather(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let args = interaction.data.options.first().map_or("", |o| {
        if let CommandDataOptionValue::String(s) = &o.value {
            s.as_str()
        } else {
            ""
        }
    });

    let (location, location_name) = match parse_location(ctx, args).await {
        Ok((l, n)) => (l, n),
        Err(e) => {
            interaction
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(e.to_string()),
                )
                .await?;
            return Ok(());
        }
    };

    let (tomorrowio_api_key, airnow_api_key) = {
        let data = ctx.data.read().await;
        #[allow(clippy::unwrap_used)]
        (
            data.get::<config::TomorrowIO>().unwrap().api_key.clone(),
            data.get::<config::AirNow>().unwrap().api_key.clone(),
        )
    };
    let conditions = tomorrowio::get_current(&location, &tomorrowio_api_key).await?;

    let aqi = airnow::get_current_aqi(&location, &airnow_api_key).await?;

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

    let response_msg = format!(
        "weather in {}
temperature | {} {}
conditions | {}
relative humidty | {} {}
wind | {} {} {}
uv index | {}
air quality index | {}",
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
        aqi.map_or_else(|| "--".to_string(), |(i, c)| format!("{i} ({c})"))
    );

    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(response_msg),
        )
        .await?;

    Ok(())
}

// Replies to msg with the hourly forecast (12h) for either the bot's location or the supplied location
// Takes a single optional argument - location as zipcode, city+state, or lat/lng in decimal form
pub async fn forecast(ctx: &Context, interaction: &CommandInteraction) -> CommandResult {
    let mut location_args = "";
    let mut hours = 6;
    for o in &interaction.data.options {
        match &o.name[..] {
            "location" => {
                if let CommandDataOptionValue::String(s) = &o.value {
                    location_args = s.as_str();
                }
            }
            "hours" => {
                if let CommandDataOptionValue::Integer(h) = o.value {
                    hours = h;
                }
            }
            _ => {}
        }
    }

    let (location, location_name) = match parse_location(ctx, location_args).await {
        Ok((l, n)) => (l, n),
        Err(e) => {
            interaction
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(e.to_string()),
                )
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
        "forecast for {location_name}\n``` Time |  Temp  |  RH  | Dewpoint |  Rain  |   Wind   | UV\n      |        |      |          |        |          |\n"
    );
    for v in forecast {
        let values = v.values;
        let time = v.start_time.with_timezone(&timezone);
        let wind_str: String = match values.wind_direction {
            None => "--".into(),
            Some(dir) => match values.wind_speed {
                None => "--".into(),
                Some(speed) => {
                    let cardinal = deg_to_cardinal(dir);
                    format!("{:<5} {cardinal}", format!("{speed:.0}mph"))
                }
            },
        };
        let mut time_str = time.format("%l%P").to_string();
        time_str.truncate(time_str.len() - 1);
        writeln!(
            response_msg,
            "{:^6}|{:^8}|{:^6}|{:^10}|{:^8}|{:^10}|{}",
            time_str,
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
                .map_or_else(|| "--".to_string(), |t| format!("{t:.0}%")),
            wind_str,
            values
                .uv_index
                .map_or_else(|| " --".into(), |t| format!(" {t:.0}"))
        )?;
    }
    write!(response_msg, "```")?;

    interaction
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(response_msg),
        )
        .await?;

    Ok(())
}

async fn parse_location(ctx: &Context, args: &str) -> Result<(Point, String), CommandError> {
    let point_regex = regex::Regex::new(r"^(-?\d+\.?\d*)[,\s]+(-?\d+\.?\d*)$")?;

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

fn deg_to_cardinal(mut deg: f32) -> &'static str {
    while deg < 0.0 {
        deg += 360.0;
    }
    while deg >= 360.0 {
        deg -= 360.0;
    }
    if deg < 22.5 {
        "N"
    } else if deg < 67.5 {
        "NE"
    } else if deg < 112.5 {
        "E"
    } else if deg < 157.5 {
        "SE"
    } else if deg < 202.5 {
        "S"
    } else if deg < 247.5 {
        "SW"
    } else if deg < 292.5 {
        "W"
    } else if deg < 337.5 {
        "NW"
    } else {
        "N"
    }
}
