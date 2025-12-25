use anyhow::Result;

#[derive(Debug, Clone)]
pub struct NewsItem {
    pub datetime: String, // ISO8601 or RFC3339
    pub headline: String,
    pub source: String,
    pub url: String,
}

pub trait NewsCollector {
    fn collect_news(&self, ticker: &str, window_days: i64) -> Result<Vec<NewsItem>>;
}

pub struct NullNewsCollector;
impl NewsCollector for NullNewsCollector {
    fn collect_news(&self, _ticker: &str, _window_days: i64) -> Result<Vec<NewsItem>> {
        Ok(vec![])
    }
}

#[derive(Debug, Clone)]
pub struct SenateEvent {
    pub date: String, // YYYY-MM-DD usually
    pub chamber: String,
    pub member_name: String,
    pub activity_type: String, // BUY, SELL, DISCLOSURE
    pub notes: Option<String>,
}

pub trait SenateCollector {
    fn collect_senate_activity(&self, ticker: &str, window_days: i64) -> Result<Vec<SenateEvent>>;
}

pub struct NullSenateCollector;
impl SenateCollector for NullSenateCollector {
    fn collect_senate_activity(&self, _ticker: &str, _window_days: i64) -> Result<Vec<SenateEvent>> {
        Ok(vec![])
    }
}

#[derive(Debug, Clone)]
pub struct FinanceSnapshot {
    pub source: String,
    pub asof_utc: String,
    pub price_last: f64,
    pub market_cap_approx: Option<f64>,
    pub pe_ratio_approx: Option<f64>,
    pub notes: String,
}

pub trait FinanceSnapshotCollector {
    fn collect_snapshot(&self, ticker: &str) -> Result<Option<FinanceSnapshot>>;
}

pub struct StubFinanceSnapshotCollector;
impl FinanceSnapshotCollector for StubFinanceSnapshotCollector {
    fn collect_snapshot(&self, _ticker: &str) -> Result<Option<FinanceSnapshot>> {
        // In the future, this would scrape or return a static placeholder
        Ok(None)
    }
}
