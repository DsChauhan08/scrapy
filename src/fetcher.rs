use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use crate::market::MinuteBar;
use std::thread;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct YahooResponse {
    chart: YahooChart,
}

#[derive(Debug, Deserialize)]
struct YahooChart {
    result: Option<Vec<YahooResult>>,
    error: Option<YahooError>,
}

#[derive(Debug, Deserialize)]
struct YahooError {
    description: String,
    code: String,
}

#[derive(Debug, Deserialize)]
struct YahooResult {
    meta: YahooMeta,
    timestamp: Option<Vec<i64>>,
    indicators: YahooIndicators,
}

#[derive(Debug, Deserialize, Clone)]
pub struct YahooMeta {
    pub currency: Option<String>,
    pub symbol: String,
    pub regularMarketPrice: Option<f64>,
    pub chartPreviousClose: Option<f64>,
    // These might not be in chart meta, but let's check. 
    // Usually chart meta has: currency, symbol, regularMarketPrice, gmtoffset.
    // Full quote is often not here, but basic price is.
}

#[derive(Debug, Deserialize)]
struct YahooIndicators {
    quote: Vec<YahooQuote>,
}

#[derive(Debug, Deserialize)]
struct YahooQuote {
    open: Vec<Option<f64>>,
    high: Vec<Option<f64>>,
    low: Vec<Option<f64>>,
    close: Vec<Option<f64>>,
    volume: Vec<Option<u64>>,
}

// Return both bars AND metadata
pub fn fetch_minute_bars(ticker: &str, days: i64) -> Result<(Vec<MinuteBar>, Option<YahooMeta>)> {
    let range = "5d"; 
    let urls = vec![
        format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1m&range={}", ticker, range),
        format!("https://query2.finance.yahoo.com/v8/finance/chart/{}?interval=1m&range={}", ticker, range),
    ];

    let mut last_err = anyhow::anyhow!("No URLs tried");

    for (i, url) in urls.iter().enumerate() {
        if i > 0 {
            thread::sleep(Duration::from_secs(1));
        }

        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()?;

        let resp_res = client.get(url).send();
        
        match resp_res {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let text = resp.text()?;
                    let y_resp: YahooResponse = serde_json::from_str(&text).with_context(|| "Failed to parse Yahoo JSON")?;
                    
                    if let Some(res_list) = y_resp.chart.result {
                        if !res_list.is_empty() {
                            let bars = parse_yahoo_result(&res_list[0])?;
                            let meta = res_list[0].meta.clone();
                            return Ok((bars, Some(meta)));
                        }
                    }
                    if let Some(err) = y_resp.chart.error {
                        last_err = anyhow::anyhow!("Yahoo API Error: {} ({})", err.description, err.code);
                    }
                } else {
                    last_err = anyhow::anyhow!("Request failed with status: {}", status);
                }
            },
            Err(e) => {
                last_err = anyhow::anyhow!("Network error: {}", e);
            }
        }
    }
    
    Err(last_err)
}

fn parse_yahoo_result(data: &YahooResult) -> Result<Vec<MinuteBar>> {
    let timestamps = match &data.timestamp {
        Some(t) => t,
        None => return Ok(vec![])
    };
    
    if data.indicators.quote.is_empty() {
        return Ok(vec![]); 
    }
    let quote = &data.indicators.quote[0];

    let mut bars = Vec::with_capacity(timestamps.len());
    
    for (i, &ts_secs) in timestamps.iter().enumerate() {
        if let (Some(o), Some(h), Some(l), Some(c), Some(v)) = (
            quote.open.get(i).and_then(|x| *x),
            quote.high.get(i).and_then(|x| *x),
            quote.low.get(i).and_then(|x| *x),
            quote.close.get(i).and_then(|x| *x),
            quote.volume.get(i).and_then(|x| *x),
        ) {
             let ts_utc = Utc.timestamp_opt(ts_secs, 0).single().ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?;
            
            bars.push(MinuteBar {
                ts_utc,
                o,
                h,
                l,
                c,
                v,
            });
        }
    }
    Ok(bars)
}
