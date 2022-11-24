use chrono::{DateTime, Utc};
use serde::Deserialize;
use serenity::http::client::Http;
use serenity::model::id::ChannelId;
use sqlx::{Pool, Postgres};
use std::fmt;
use std::sync::Arc;
use tracing::{debug, error, info};

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

pub async fn poll_shipments_loop(discord_http: Arc<Http>, db: Pool<Postgres>, api_key: String) {
    info!("starting shipment poller");
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60 * 15));

    loop {
        interval.tick().await;

        let rows = match sqlx::query!("SELECT carrier::text AS carrier, tracking_number, status::text AS status, author_id, channel_id, comment FROM shipment WHERE status = ANY('{transit, pre_transit, unknown}')").fetch_all(&db).await {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "error getting shipments from db");
                continue;
            }
        };
        debug!(shipments = rows.len(), "polling for shipments");
        for row in rows {
            use TrackingNumber::*;
            let old_status = if let Some(s) = &row.status {
                s
            } else {
                error!(?row, "missing status in shipment polling");
                continue;
            };
            let carrier = if let Some(c) = &row.carrier {
                c
            } else {
                error!(?row, "missing carrier on shipment row");
                continue;
            };
            let tracking_number = match carrier.as_str() {
                "fedex" => FedEx(row.tracking_number.clone()),
                "ups" => Ups(row.tracking_number.clone()),
                "usps" => Usps(row.tracking_number.clone()),
                _ => {
                    error!(
                        carrier = row.carrier,
                        "unrecognized carrier in shipment polling"
                    );
                    continue;
                }
            };
            let new_status = match get_tracking_status(&tracking_number, &api_key).await {
                Ok(s) => s,
                Err(e) => {
                    error!(error = %e, %tracking_number, "error polling shipment");
                    continue;
                }
            };
            if let Some(tracking_status) = new_status.tracking_status {
                if old_status != &tracking_status.status.to_string() {
                    if tracking_status.status == Status::Delivered {
                        let comment = if let Some(c) = row.comment {
                            format!(" ({}) ", c)
                        } else {
                            String::from(" ")
                        };
                        let channel_id = match u64::try_from(row.channel_id) {
                            Ok(c) => ChannelId(c),
                            Err(e) => {
                                error!(error = %e, channel_id = row.channel_id, "unable to convert channel id");
                                continue;
                            }
                        };
                        if let Err(e) = channel_id.say(&discord_http, format!("<@{}>: Your {} shipment {}{}was marked as delivered at {} with the following message: {}", row.author_id, carrier, row.tracking_number, comment, tracking_status.status_date, tracking_status.status_details)).await {
                            error!(error = %e, "error alerting user of shipment");
                            continue;
                        }
                    }

                    #[allow(clippy::panic)]
                    if let Err(e) = sqlx::query!("UPDATE shipment SET status = 'delivered' WHERE carrier = $1::shipment_carrier AND tracking_number = $2 AND status <> 'delivered'", &row.carrier as _, row.tracking_number).fetch_optional(&db).await {
                        error!(error = %e, "error updating polled shipment");
                        continue;
                    }
                }
            }
        }
    }
}
