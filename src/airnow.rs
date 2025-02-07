use crate::model::Point;
use serde::Deserialize;
use std::cmp::Ordering;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Observation {
    #[serde(rename = "AQI")]
    aqi: i32,
    category: Category,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Category {
    number: i32,
    name: String,
}

impl Ord for Category {
    fn cmp(&self, other: &Self) -> Ordering {
        self.number.cmp(&other.number)
    }
}

impl PartialOrd for Category {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub async fn get_current_aqi(
    location: &Point,
    api_key: &str,
) -> Result<Option<(i32, String)>, reqwest::Error> {
    let Point { lat, lng } = location;
    let client = reqwest::Client::new();

    let resp = client.get(format!("https://www.airnowapi.org/aq/observation/latLong/current/?format=application/json&latitude={lat}&longitude={lng}&API_KEY={api_key}")).send().await?;
    let observations = resp.error_for_status()?.json::<Vec<Observation>>().await?;

    let mut max: Option<(i32, Category)> = None;
    for o in observations {
        if let Some(ref mut m) = max {
            if o.aqi > m.0 {
                m.0 = o.aqi;
                m.1 = o.category;
            }
        } else {
            max = Some((o.aqi, o.category));
        }
    }

    Ok(max.map(|(aqi, cat)| (aqi, cat.name)))
}
