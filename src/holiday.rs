use chrono::Datelike;
use log::{info, warn};

/// Fetch Chinese holidays from NateScarlet/holiday-cn (GitHub).
/// Returns Vec of (date_str "YYYY-MM-DD", htype "rest"|"overtime", note).
pub async fn fetch_year(year: i32) -> Vec<(String, String, String)> {
    let url = format!(
        "https://cdn.jsdelivr.net/gh/NateScarlet/holiday-cn@master/{}.json",
        year
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!("Holiday API request failed for {}: {}", year, e);
            return vec![];
        }
    };

    // Check HTTP status
    if !resp.status().is_success() {
        warn!("Holiday API returned {} for year {}", resp.status(), year);
        return vec![];
    }

    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            warn!("Holiday API parse failed for {}: {}", year, e);
            return vec![];
        }
    };

    let days = match json.get("days").and_then(|v| v.as_array()) {
        Some(d) => d,
        None => {
            warn!("Holiday API missing 'days' field for {}", year);
            return vec![];
        }
    };

    let mut result = Vec::new();
    for day in days {
        let date = day.get("date").and_then(|v| v.as_str()).unwrap_or("");
        let name = day.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let is_off = day.get("isOffDay").and_then(|v| v.as_bool()).unwrap_or(true);
        let htype = if is_off { "rest" } else { "overtime" };
        if !date.is_empty() {
            result.push((date.to_string(), htype.to_string(), name.to_string()));
        }
    }

    info!("Fetched {} holiday entries for year {}", result.len(), year);
    result
}

/// Fetch holidays for current year and next year.
pub async fn fetch_current_and_next() -> Vec<(String, String, String)> {
    let now = chrono::Local::now();
    let year = now.year();
    let mut entries = fetch_year(year).await;
    entries.extend(fetch_year(year + 1).await);
    entries
}
