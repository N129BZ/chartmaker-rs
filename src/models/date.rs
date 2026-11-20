use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct ChartDatesWrapper {
    #[serde(rename = "ChartDates")]
    chart_dates: Vec<String>,
}

/// Parses "MM-DD-YYYY" format into Chrono NaiveDate
pub fn parse_chart_date_string(date_str: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date_str, "%m-%d-%Y").ok()
}

/// Finds the most relevant 28/56 day FAA cycle date string from chartdates.json.
/// Returns the latest past/current cycle date, matching make.js `getBestChartDate()`.
pub fn get_best_chart_date(chartdates_path: &Path) -> Result<String> {
    let raw = fs::read_to_string(chartdates_path)
        .with_context(|| format!("Failed to read chartdates file at {:?}", chartdates_path))?;
    let wrapper: ChartDatesWrapper = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse chartdates.json at {:?}", chartdates_path))?;

    let today = chrono::Local::now().date_naive();

    let mut dates: Vec<(NaiveDate, String)> = wrapper
        .chart_dates
        .into_iter()
        .filter_map(|s| parse_chart_date_string(&s).map(|d| (d, s)))
        .collect();

    // Sort descending (b.cmp(a))
    dates.sort_by(|a, b| b.0.cmp(&a.0));

    // Find the latest date that is <= today
    for (d, str_val) in &dates {
        if *d <= today {
            return Ok(str_val.clone());
        }
    }

    // Fallback to earliest available if all are in the future
    if let Some((_, str_val)) = dates.last() {
        return Ok(str_val.clone());
    }

    Ok("01-01-2025".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_parse_date() {
        let d = parse_chart_date_string("11-20-2025").unwrap();
        assert_eq!(d.year(), 2025);
        assert_eq!(d.month(), 11);
        assert_eq!(d.day(), 20);
    }

    #[test]
    fn test_best_chart_date_real_file() {
        let date_str = get_best_chart_date(Path::new("chartdates.json")).unwrap();
        assert!(parse_chart_date_string(&date_str).is_some());
    }
}
