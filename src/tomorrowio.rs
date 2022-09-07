use crate::model::Point;
use chrono::{DateTime, Utc};
use reqwest::Url;
use serde::Deserialize;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Reqwest(reqwest::Error),
    MissingTimelines,
    MissingIntervals,
    InvalidInterval(&'static str, String),
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Reqwest(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::MissingTimelines => write!(f, "response missing timelines"),
            Error::MissingIntervals => write!(f, "timeline missing intervals"),
            Error::InvalidInterval(expected, actual) => {
                write!(f, "expect timeline interval {}, got {}", expected, actual)
            }
            Error::Reqwest(e) => write!(f, "request error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Deserialize)]
struct ApiResponse {
    data: ApiData,
}

#[derive(Deserialize)]
struct ApiData {
    timelines: Vec<Timeline>,
}

#[derive(Deserialize)]
struct Timeline {
    timestep: String,
    intervals: Vec<Interval>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interval {
    pub start_time: DateTime<Utc>,
    pub values: Values,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Values {
    pub temperature: Option<f32>,
    pub temperature_apparent: Option<f32>,
    pub humidity: Option<f32>,
    pub dew_point: Option<f32>,
    pub wind_speed: Option<f32>,
    pub wind_direction: Option<f32>,
    pub wind_gust: Option<f32>,
    pub uv_index: Option<u32>,
    pub weather_code: Option<u32>,
    pub epa_index: Option<u32>,
    pub tree_index: Option<u32>,
    pub grass_index: Option<u32>,
    pub weed_index: Option<u32>,
    pub precipitation_probability: Option<f32>,
}

pub async fn get_current(location: &Point, api_key: &str) -> Result<Values, Error> {
    let client = reqwest::Client::new();

    let resp = client.get(Url::parse(&format!("https://api.tomorrow.io/v4/timelines?location={}&fields=temperature,temperatureApparent,humidity,dewPoint,windSpeed,windDirection,windGust,uvIndex,weatherCode,epaIndex,treeIndex,grassIndex,weedIndex&timesteps=current&units=imperial&apikey={}", location, api_key)).unwrap()).send().await?;
    let api_response = match resp.error_for_status() {
        Ok(resp) => resp.json::<ApiResponse>().await?,
        Err(e) => return Err(Error::from(e)),
    };
    let timelines = api_response.data.timelines;
    let timeline = match timelines.into_iter().next() {
        Some(t) => t,
        None => return Err(Error::MissingTimelines),
    };
    if timeline.timestep != "current" {
        return Err(Error::InvalidInterval("current", timeline.timestep));
    }
    let interval = match timeline.intervals.into_iter().next() {
        Some(i) => i,
        None => return Err(Error::MissingIntervals),
    };
    Ok(interval.values)
}

pub async fn get_hourly(
    location: &Point,
    api_key: &str,
    hours: i64,
) -> Result<Vec<Interval>, Error> {
    let client = reqwest::Client::new();

    let resp = client.get(Url::parse(&format!("https://api.tomorrow.io/v4/timelines?location={}&startTime=now&endTime=nowPlus{}h&fields=temperature,humidity,dewPoint,precipitationProbability&timesteps=1h&units=imperial&apikey={}", location, hours, api_key)).unwrap()).send().await?;
    let api_response = match resp.error_for_status() {
        Ok(resp) => resp.json::<ApiResponse>().await?,
        Err(e) => return Err(Error::from(e)),
    };
    let timelines = api_response.data.timelines;
    let timeline = match timelines.into_iter().next() {
        Some(t) => t,
        None => return Err(Error::MissingTimelines),
    };
    if timeline.timestep != "1h" {
        return Err(Error::InvalidInterval("1h", timeline.timestep));
    }
    let intervals = timeline.intervals;
    if intervals.is_empty() {
        return Err(Error::MissingIntervals);
    }
    Ok(intervals)
}
