use chrono::{DateTime, NaiveDate, NaiveDateTime, Timelike, Utc};
use chrono_tz::America::New_York;
use chrono_tz::Tz;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct MinuteBar {
    pub ts_utc: DateTime<Utc>,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: u64,
}

#[derive(Debug, Clone)]
pub struct HourBar {
    pub ts_local: String, // RFC3339 in America/New_York
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: u64,
}

#[derive(Debug, Clone)]
pub struct PriceChart1H {
    pub ticker: String,
    pub window_days: i64,
    pub bars: Vec<HourBar>,
}

/// Resamples minute bars into 1-hour bars for the regular US session (09:30-16:00 ET).
/// Only the last `window_days` trading days are included.
pub fn resample_1h_regular_session(ticker: &str, minutes: &[MinuteBar], window_days: i64) -> PriceChart1H {
    // 1. Group strictly VALID bars by Trading Day (Local Date)
    // Using BTreeMap to keep days sorted
    let mut by_day: BTreeMap<NaiveDate, Vec<&MinuteBar>> = BTreeMap::new();
    for b in minutes {
        let local = b.ts_utc.with_timezone(&New_York);
        if is_regular_session(&local) {
             by_day.entry(local.date_naive()).or_default().push(b);
        }
    }

    // 2. Select last N days
    let days: Vec<NaiveDate> = by_day.keys().cloned().collect();
    let start_idx = if days.len() > window_days as usize {
        days.len() - window_days as usize
    } else {
        0
    };
    let keep_days = &days[start_idx..];

    // 3. Resample each day into hourly buckets
    let mut final_bars = Vec::new();

    for day in keep_days {
        if let Some(day_minutes) = by_day.get(day) {
             // Map BucketStart -> HourBar. BTreeMap ensures chronological order (09:30, 10:30, ...)
             let mut day_buckets: BTreeMap<DateTime<Tz>, HourBar> = BTreeMap::new();
             
             for b in day_minutes {
                 let local = b.ts_utc.with_timezone(&New_York);
                 // Safety: is_regular_session already checked, so get_bucket_start shouldn't fail
                 if let Some(bucket_start) = get_bucket_start(&local) {
                     day_buckets
                        .entry(bucket_start)
                        .and_modify(|agg| {
                            agg.h = agg.h.max(b.h);
                            agg.l = agg.l.min(b.l);
                            agg.c = b.c;   // Last bar processed becomes the close
                            agg.v += b.v;
                        })
                        .or_insert(HourBar {
                            ts_local: bucket_start.to_rfc3339(),
                            o: b.o,
                            h: b.h,
                            l: b.l,
                            c: b.c,
                            v: b.v,
                        });
                 }
             }
             
             // Append to final list in order
             for (_, bar) in day_buckets {
                 final_bars.push(bar);
             }
        }
    }

    PriceChart1H {
        ticker: ticker.to_uppercase(),
        window_days,
        bars: final_bars,
    }
}

/// Returns true if the time is within 09:30:00 (inclusive) and 16:00:00 (exclusive).
fn is_regular_session(dt: &DateTime<Tz>) -> bool {
    let h = dt.hour();
    let m = dt.minute();
    // Pre-market: before 09:30
    if h < 9 || (h == 9 && m < 30) { return false; }
    // After-hours: 16:00 and later
    if h >= 16 { return false; }
    true
}

/// Returns the start time of the 1-hour bucket (e.g., 09:30, 10:30).
fn get_bucket_start(dt: &DateTime<Tz>) -> Option<DateTime<Tz>> {
    let h = dt.hour();
    let m = dt.minute();
    
    // Calculate minutes since 09:30
    let minutes_since_930 = (h as i32 - 9) * 60 + (m as i32 - 30);
    // Bucket index (0 for 09:30-10:29, 1 for 10:30-11:29, etc.)
    let bucket_idx = minutes_since_930.div_euclid(60); 
    
    // Reconstruct start time
    let start_minutes_from_midnight = 9 * 60 + 30 + bucket_idx * 60;
    
    let start_h = (start_minutes_from_midnight / 60) as u32;
    let start_m = (start_minutes_from_midnight % 60) as u32;
    
    let naive = NaiveDateTime::new(dt.date_naive(), chrono::NaiveTime::from_hms_opt(start_h, start_m, 0)?);
    naive.and_local_timezone(New_York).single()
}
