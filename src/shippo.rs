use chrono::{DateTime, Utc};
use reqwest::Url;
use serde::Deserialize;
use std::fmt;

pub enum TrackingNumber {
    FedEx(String),
    Ups(String),
    Usps(String),
}

impl fmt::Display for TrackingNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(clippy::enum_glob_use)]
        use TrackingNumber::*;
        match self {
            FedEx(t) => write!(f, "fedex/{}", t),
            Ups(t) => write!(f, "ups/{}", t),
            Usps(t) => write!(f, "usps/{}", t),
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

#[derive(Debug, Deserialize)]
pub struct TrackingResponse {
    pub eta: Option<DateTime<Utc>>,
    pub tracking_status: Option<TrackingStatus>,
}

#[derive(Debug, Deserialize)]
pub struct TrackingStatus {
    pub status: Status,
    pub status_details: String,
    pub status_date: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TrackUpdatedRequest {
    pub event: String,
    pub data: TrackingResponse,
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
        Err(e) => Err(e),
    }
}

pub fn handle_track_updated_webhook(body: TrackUpdatedRequest) {
    println!("{:?}", body);
}
