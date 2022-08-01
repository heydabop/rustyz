use chrono::{DateTime, Utc};
use reqwest::Url;
use serde::Deserialize;
use std::fmt;

pub enum TrackingNumber {
    FedEx(String),
    UPS(String),
    USPS(String),
}

impl fmt::Display for TrackingNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TrackingNumber::*;
        match self {
            FedEx(t) => write!(f, "fedex/{}", t),
            UPS(t) => write!(f, "ups/{}", t),
            USPS(t) => write!(f, "usps/{}", t),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Unknown,
    PreTransit,
    Transit,
    Delivered,
    Returned,
    Failure,
}

#[derive(Deserialize)]
pub struct TrackingResponse {
    pub eta: Option<DateTime<Utc>>,
    pub tracking_status: TrackingStatus,
}

#[derive(Deserialize)]
pub struct TrackingStatus {
    pub status: Status,
    pub status_details: String,
}

pub async fn get_tracking_status(
    tracking_number: &TrackingNumber,
    api_key: &str,
) -> Result<TrackingResponse, reqwest::Error> {
    let client = reqwest::Client::new();

    let response = client
        .get(
            Url::parse(&format!(
                "https://api.goshippo.com/tracks/{}/",
                tracking_number
            ))
            .unwrap(),
        )
        .header("Authorization", format!("ShippoToken {}", api_key))
        .send()
        .await?;
    match response.error_for_status() {
        Ok(r) => Ok(r.json::<TrackingResponse>().await?),
        Err(e) => return Err(e),
    }
}
