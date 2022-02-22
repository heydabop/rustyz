use reqwest::Url;
use serde::Deserialize;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Reqwest(reqwest::Error),
    MissingTimelines,
    MissingIntervals,
    NotCurrent,
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
            Error::NotCurrent => write!(f, "timeline interval not 'current'"),
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
struct Interval {
    values: Values,
}

#[derive(Deserialize, Clone, Copy)]
pub struct Values {
    pub temperature: Option<f32>,
    #[serde(rename = "temperatureApparent")]
    pub temperature_apparent: Option<f32>,
    pub humidity: Option<f32>,
    #[serde(rename = "dewPoint")]
    pub dewpoint: Option<f32>,
    #[serde(rename = "windSpeed")]
    pub wind_speed: Option<f32>,
    #[serde(rename = "windDirection")]
    pub wind_direction: Option<f32>,
    #[serde(rename = "windGust")]
    pub wind_gust: Option<f32>,
    #[serde(rename = "uvIndex")]
    pub uv_index: Option<u32>,
    #[serde(rename = "weatherCode")]
    pub weather_code: Option<u32>,
    #[serde(rename = "epaIndex")]
    pub epa_index: Option<u32>,
}

pub async fn get_current(location: &str, api_key: &str) -> Result<Values, Error> {
    let client = reqwest::Client::new();

    let resp = client.get(Url::parse(&format!("https://api.tomorrow.io/v4/timelines?location={}&fields=temperature,temperatureApparent,humidity,dewPoint,windSpeed,windDirection,windGust,uvIndex,weatherCode,epaIndex&timesteps=current&units=imperial&apikey={}", location, api_key)).unwrap()).send().await?;
    let api_response = match resp.error_for_status() {
        Ok(resp) => resp.json::<ApiResponse>().await?,
        Err(e) => return Err(Error::from(e)),
    };
    let timelines = api_response.data.timelines;
    if timelines.is_empty() {
        return Err(Error::MissingTimelines);
    }
    if timelines[0].timestep != "current" {
        return Err(Error::NotCurrent);
    }
    let intervals = &timelines[0].intervals;
    if intervals.is_empty() {
        return Err(Error::MissingIntervals);
    }
    Ok(intervals[0].values)
}
