use crate::model::Point;
use reqwest::Url;
use serde::Deserialize;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Reqwest(reqwest::Error),
    Status(String),
    NoResults,
    MissingGeometry,
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Reqwest(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Status(status) => write!(f, "expected OK status, got {}", status),
            Error::NoResults => write!(f, "0 geocoding results"),
            Error::MissingGeometry => write!(f, "missing geometry in geocoding result"),
            Error::Reqwest(e) => write!(f, "request error: {}", e),
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

pub async fn geocode(address: &str, api_key: &str) -> Result<(Point, Option<String>), Error> {
    let client = reqwest::Client::new();

    let resp = client
        .get(
            Url::parse(&format!(
                "https://maps.googleapis.com/maps/api/geocode/json?&address={}&key={}",
                address, api_key
            ))
            .unwrap(),
        )
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
