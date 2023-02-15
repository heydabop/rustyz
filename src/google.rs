use crate::model::Point;
use chrono_tz::Tz;
use serde::Deserialize;
use std::fmt;
use std::str::FromStr;

#[derive(Debug)]
pub enum Error {
    Reqwest(reqwest::Error),
    Status(String),
    NoResults,
    MissingGeometry,
    InvalidTz(String),
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Reqwest(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Status(status) => write!(f, "expected OK status, got {status}"),
            Error::NoResults => write!(f, "0 geocoding results"),
            Error::MissingGeometry => write!(f, "missing geometry in geocoding result"),
            Error::Reqwest(e) => write!(f, "request error: {e}"),
            Error::InvalidTz(e) => write!(f, "invalid timezone ID: {e}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Deserialize)]
struct GeocodeResponse {
    results: Vec<GeocodeResult>,
    status: String,
}

#[derive(Deserialize)]
struct GeocodeResult {
    address_components: Option<Vec<Address>>,
    geometry: GeocodeGeometry,
}

#[derive(Deserialize)]
struct GeocodeGeometry {
    location: Option<Point>,
}

#[derive(Deserialize)]
struct Address {
    long_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimezoneResponse {
    status: String,
    time_zone_id: Option<String>,
}

pub async fn geocode(address: &str, api_key: &str) -> Result<(Point, Option<String>), Error> {
    let client = reqwest::Client::new();

    let resp = client
        .get(&format!(
            "https://maps.googleapis.com/maps/api/geocode/json?&address={address}&key={api_key}"
        ))
        .send()
        .await?;
    let geo = match resp.error_for_status() {
        Ok(resp) => resp.json::<GeocodeResponse>().await?,
        Err(e) => return Err(Error::from(e)),
    };
    if geo.status != "OK" {
        return Err(Error::Status(geo.status));
    }
    if geo.results.is_empty() {
        return Err(Error::NoResults);
    }
    match geo.results[0].geometry.location {
        Some(p) => {
            let location_name = geo.results[0]
                .address_components
                .as_ref()
                .and_then(|a| a.get(0).and_then(|c| c.long_name.clone()));
            Ok((p, location_name))
        }
        None => Err(Error::MissingGeometry),
    }
}

pub async fn timezone(location: &Point, timestamp: i64, api_key: &str) -> Result<Tz, Error> {
    let client = reqwest::Client::new();

    let resp = client
        .get(&format!(
            "https://maps.googleapis.com/maps/api/timezone/json?location={location}&timestamp={timestamp}&key={api_key}"
        ))
        .send()
        .await?;
    let json = match resp.error_for_status() {
        Ok(resp) => resp.json::<TimezoneResponse>().await?,
        Err(e) => return Err(Error::from(e)),
    };
    if json.status != "OK" {
        return Err(Error::Status(json.status));
    }
    match json.time_zone_id {
        Some(tz_id) => match Tz::from_str(&tz_id) {
            Ok(tz) => Ok(tz),
            Err(e) => Err(Error::InvalidTz(e.to_string())),
        },
        None => Err(Error::NoResults),
    }
}
