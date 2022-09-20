use chrono::{DateTime, Utc};
use serde::Deserialize;
use serenity::http::client::Http;
use serenity::json::json;
use sqlx::{Pool, Postgres};
use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;
use warp::http::StatusCode;

pub enum TrackingNumber {
    FedEx(String),
    Ups(String),
    Usps(String),
}

impl TrackingNumber {
    pub fn carrier(&self) -> &'static str {
        use TrackingNumber::*;
        match self {
            FedEx(_) => "fedex",
            Ups(_) => "ups",
            Usps(_) => "usps",
        }
    }

    pub fn number(&self) -> String {
        use TrackingNumber::*;
        match self {
            FedEx(n) | Ups(n) | Usps(n) => n.clone(),
        }
    }
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

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Unknown,
    PreTransit,
    Transit,
    Delivered,
    Returned,
    Failure,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(clippy::enum_glob_use)]
        use Status::*;
        match self {
            Unknown => write!(f, "unknown"),
            PreTransit => write!(f, "pre_transit"),
            Transit => write!(f, "transit"),
            Delivered => write!(f, "delivered"),
            Returned => write!(f, "returned"),
            Failure => write!(f, "failure"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TrackingResponse {
    pub carrier: Option<String>,
    pub tracking_number: String,
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
    pub carrier: Option<String>,
}

pub async fn get_tracking_status(
    tracking_number: &TrackingNumber,
    api_key: &str,
) -> Result<TrackingResponse, reqwest::Error> {
    let client = reqwest::Client::new();

    let response = client
        .post("https://api.goshippo.com/tracks/")
        .header("Authorization", format!("ShippoToken {}", api_key))
        .form(&[
            ("carrier", tracking_number.carrier()),
            ("tracking_number", &tracking_number.number()),
        ])
        .send()
        .await?;
    match response.error_for_status() {
        Ok(r) => Ok(r.json::<TrackingResponse>().await?),
        Err(e) => Err(e),
    }
}

pub async fn handle_track_updated_webhook(
    body: TrackUpdatedRequest,
    db: Pool<Postgres>,
    http: Arc<Http>,
) -> Result<(), Box<dyn std::error::Error>> {
    if body.event != "track_updated" {
        println!("non track_updated event: {}", body.event);
        return Ok(());
    }
    if body.data.tracking_status.is_none() {
        println!("missing tracking_status");
        return Ok(());
    }
    let tracking = body.data.tracking_status.unwrap();
    if tracking.status == Status::Delivered {
        let row = sqlx::query!("UPDATE shipment SET status = 'delivered' WHERE carrier = $1::shipment_carrier AND tracking_number = $2 AND status <> 'delivered' RETURNING author_id, channel_id", body.carrier.clone().or_else(|| body.data.carrier.clone()) as _, body.data.tracking_number as _).fetch_optional(&db).await?;
        if let Some(row) = row {
            #[allow(clippy::cast_sign_loss)]
            http.send_message(row.channel_id as u64, &json!({"content": format!("<@{}>: Your {} shipment {} was marked as delivered at {} with the following message: {}", row.author_id, &body.carrier.or(body.data.carrier).or_else(|| Some(String::new())).unwrap(), &body.data.tracking_number, tracking.status_date, tracking.status_details)})).await?;
        } else {
            println!("shipment not found {}", body.data.tracking_number);
        }
    }
    Ok(())
}

pub async fn handle_post(
    body: TrackUpdatedRequest,
    db: Pool<Postgres>,
    http: Arc<Http>,
) -> Result<impl warp::Reply, Infallible> {
    println!("shippo webhook: {:?}", body);
    if let Err(e) = handle_track_updated_webhook(body, db, http).await {
        eprintln!("shippo webbook error: {}", e);
        return Ok(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(StatusCode::NO_CONTENT)
}
