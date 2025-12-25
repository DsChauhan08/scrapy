use anyhow::{Context, Result};
use std::time::Duration;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::escape::unescape;
use serde::Deserialize;
use scraper::{Html, Selector}; 
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE};

#[derive(Debug, Clone)]
pub struct NewsItem {
    pub datetime: String,
    pub headline: String,
    pub source: String,
    pub content_snippet: String, 
}

pub trait NewsCollector {
    fn collect_news(&self, ticker: &str, window_days: i64) -> Result<Vec<NewsItem>>;
}

pub struct GoogleNewsCollector;
impl NewsCollector for GoogleNewsCollector {
    fn collect_news(&self, ticker: &str, _window_days: i64) -> Result<Vec<NewsItem>> {
        let url = format!("https://news.google.com/rss/search?q={}+stock&hl=en-US&gl=US&ceid=US:en", ticker);

        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(8)) 
            .build()?;
        
        let resp = client.get(&url).send()?;
        if !resp.status().is_success() {
             return Ok(vec![]);
        }
        let xml_content = resp.text()?;
        
        let mut reader = Reader::from_str(&xml_content);
        reader.trim_text(true);

        let mut raw_items = Vec::new();
        let mut buf = Vec::new();
        let mut in_item = false;
        
        let mut current_title = String::new();
        let mut current_link = String::new();
        let mut current_date = String::new();
        let mut current_source = String::new();
        let mut current_desc = String::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    match e.name().as_ref() {
                        b"item" => in_item = true,
                        b"title" if in_item => current_title = reader.read_text(e.name())?.to_string(),
                        b"link" if in_item => current_link = reader.read_text(e.name())?.to_string(),
                        b"pubDate" if in_item => current_date = reader.read_text(e.name())?.to_string(),
                        b"source" if in_item => current_source = reader.read_text(e.name())?.to_string(),
                        b"description" if in_item => current_desc = reader.read_text(e.name())?.to_string(),
                        _ => (),
                    }
                }
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == b"item" {
                        if !current_link.is_empty() {
                            // CLEANUP DESCRIPTION
                            // 1. Unescape HTML entities (e.g. &lt; -> <)
                            let unescaped = unescape(&current_desc).unwrap_or(std::borrow::Cow::Borrowed(&current_desc));
                            // 2. Parse as HTML fragment to strip tags
                            let frag = Html::parse_fragment(&unescaped);
                            let clean_desc = frag.root_element().text().collect::<Vec<_>>().join(" ");
                            let clean_desc = clean_desc.trim().to_string();

                            raw_items.push((current_date.clone(), current_title.clone(), current_source.clone(), current_link.clone(), clean_desc));
                        }
                        in_item = false;
                        current_title.clear();
                        current_link.clear();
                        current_date.clear();
                        current_source.clear();
                        current_desc.clear();
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => (),
            }
            buf.clear();
        }

        let mut final_news = Vec::new();
        
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

        let article_client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
            .default_headers(headers)
            .timeout(Duration::from_secs(5)) 
            .redirect(reqwest::redirect::Policy::limited(10)) 
            .cookie_store(true)
            .build()?;

        for (date, title, source, link, desc) in raw_items.into_iter().take(5) { 
             let mut snippet = scrape_article_body(&article_client, &link).unwrap_or_default();
             
             // Check if scrape failed or was rejected
             if snippet.len() < 50 || snippet.contains("JavaScript is disabled") {
                 // FALLBACK: Use CLEANED RSS Description
                 if !desc.is_empty() {
                     snippet = format!("(Summary): {}", desc);
                 } else {
                     snippet = "Content unavailable.".to_string();
                 }
             }

             final_news.push(NewsItem {
                 datetime: date,
                 headline: title,
                 source: if source.is_empty() { "Google News".to_string() } else { source },
                 content_snippet: snippet,
             });
        }

        Ok(final_news)
    }
}

fn scrape_article_body(client: &reqwest::blocking::Client, url: &str) -> Result<String> {
    if url.contains("google.com/search") { return Ok("Skipped search link".to_string()); }

    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Ok(String::new());
    }
    let html = resp.text()?;
    let document = Html::parse_document(&html);
    
    let p_selector = Selector::parse("p").unwrap();
    let paragraphs: Vec<String> = document.select(&p_selector)
        .filter_map(|el| {
            let text = el.text().collect::<Vec<_>>().join(" ");
            let clean_text = text.trim();

            if clean_text.len() < 50 { return None; } 
            
            let lower = clean_text.to_lowercase();
            if lower.contains("cookie") || 
               lower.contains("subscribe") || 
               lower.contains("rights reserved") ||
               lower.contains("click here") ||
               lower.contains("javascript") ||
               lower.contains("adblock") ||
               lower.contains("promo") {
                return None;
            }
            
            Some(clean_text.to_string())
        })
        .collect();

    if paragraphs.is_empty() {
        return Ok(String::new());
    }

    let mut result = String::new();
    let mut seen = std::collections::HashSet::new();
    let mut count = 0;
    
    for p in &paragraphs {
        if seen.contains(p) { continue; }
        seen.insert(p.clone());
        
        result.push_str(p);
        result.push_str("\n\n");
        count += 1;
        if count >= 2 { break; }
    }
    
    if let Some(last) = paragraphs.last() {
        if !seen.contains(last) {
             result.push_str(last);
        }
    }

    Ok(result)
}

// ... Rest unchanged ...
#[derive(Debug, Clone)]
pub struct InsiderEvent { pub date: String, pub entity_name: String, pub relation: String, pub transaction_type: String, pub value_approx: String }
#[derive(Debug, Clone)]
pub struct InstitutionalEvent { pub holder_name: String, pub pct_held: String }
pub trait InsiderCollector {
    fn collect_activity(&self, ticker: &str, window_days: i64) -> Result<(Vec<InsiderEvent>, Vec<InstitutionalEvent>)>;
}
pub struct YahooInsiderCollector;
#[derive(Deserialize, Debug)]
struct QSumResponse { quoteSummary: QSumResult }
#[derive(Deserialize, Debug)]
struct QSumResult { result: Option<Vec<QSumModules>>, error: Option<serde_json::Value> }
#[derive(Deserialize, Debug)]
struct QSumModules { insiderTransactions: Option<InsiderTxModule>, institutionOwnership: Option<OwnershipModule>, fundOwnership: Option<OwnershipModule> }
#[derive(Deserialize, Debug)]
struct InsiderTxModule { transactions: Vec<InsiderTx> }
#[derive(Deserialize, Debug)]
struct InsiderTx { filerName: Option<String>, filerRelation: Option<String>, transactionText: Option<String>, startDate: Option<FmtDate>, value: Option<FmtValue> }
#[derive(Deserialize, Debug)]
struct OwnershipModule { ownershipList: Vec<OwnerEntry> }
#[derive(Deserialize, Debug)]
struct OwnerEntry { organization: Option<String>, pctHeld: Option<FmtValue> }
#[derive(Deserialize, Debug)]
struct FmtDate { fmt: Option<String> }
#[derive(Deserialize, Debug)]
struct FmtValue { fmt: Option<String>, raw: Option<f64> }
impl InsiderCollector for YahooInsiderCollector {
    fn collect_activity(&self, ticker: &str, window_days: i64) -> Result<(Vec<InsiderEvent>, Vec<InstitutionalEvent>)> {
        let url = format!("https://query2.finance.yahoo.com/v10/finance/quoteSummary/{}?modules=insiderTransactions,institutionOwnership,fundOwnership", ticker);
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
            .build()?;
        let resp = client.get(&url).send()?;
        if !resp.status().is_success() { return Ok((vec![], vec![])); }
        let text = resp.text()?;
        let data: QSumResponse = serde_json::from_str(&text).unwrap_or(QSumResponse { quoteSummary: QSumResult { result: None, error: None } });
        let mut trades = Vec::new();
        let mut holders = Vec::new();
        let cutoff_date = chrono::Utc::now().naive_utc().date() - chrono::Duration::days(window_days);
        if let Some(res_list) = data.quoteSummary.result {
            if let Some(modules) = res_list.first() {
                if let Some(tx_mod) = &modules.insiderTransactions {
                    for tx in &tx_mod.transactions {
                        let date_str = tx.startDate.as_ref().and_then(|d| d.fmt.clone()).unwrap_or_default();
                        let include = if date_str.is_empty() { false } else {
                            if let Ok(d) = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") { d >= cutoff_date } else { false }
                        };
                        if include {
                            trades.push(InsiderEvent {
                                date: date_str,
                                entity_name: tx.filerName.clone().unwrap_or("Unknown".to_string()),
                                relation: tx.filerRelation.clone().unwrap_or("Insider".to_string()),
                                transaction_type: tx.transactionText.clone().unwrap_or("Trade".to_string()),
                                value_approx: tx.value.as_ref().and_then(|v| v.fmt.clone()).unwrap_or("0".to_string()),
                            });
                        }
                    }
                }
                if let Some(inst) = &modules.institutionOwnership {
                    for own in inst.ownershipList.iter().take(5) {
                         holders.push(InstitutionalEvent {
                             holder_name: own.organization.clone().unwrap_or("Unknown".to_string()),
                             pct_held: own.pctHeld.as_ref().and_then(|v| v.fmt.clone()).unwrap_or("0%".to_string()),
                         });
                    }
                }
                if let Some(fund) = &modules.fundOwnership {
                    for own in fund.ownershipList.iter().take(5) {
                         holders.push(InstitutionalEvent {
                             holder_name: own.organization.clone().unwrap_or("Unknown Fund".to_string()),
                             pct_held: own.pctHeld.as_ref().and_then(|v| v.fmt.clone()).unwrap_or("0%".to_string()),
                         });
                    }
                }
            }
        }
        Ok((trades, holders))
    }
}
#[derive(Debug, Clone)]
pub struct FinanceSnapshot { pub source: String, pub asof_utc: String, pub price_last: f64, pub market_cap_approx: Option<f64>, pub pe_ratio_approx: Option<f64>, pub notes: String }
pub trait FinanceSnapshotCollector { fn collect_snapshot(&self, ticker: &str, meta: Option<&crate::fetcher::YahooMeta>) -> Result<Option<FinanceSnapshot>>; }
pub struct YahooSnapshotCollector;
impl FinanceSnapshotCollector for YahooSnapshotCollector {
    fn collect_snapshot(&self, _ticker: &str, meta: Option<&crate::fetcher::YahooMeta>) -> Result<Option<FinanceSnapshot>> {
        if let Some(m) = meta {
            return Ok(Some(FinanceSnapshot {
                source: "YahooChartMeta".to_string(),
                asof_utc: chrono::Utc::now().to_rfc3339(),
                price_last: m.regularMarketPrice.or(m.chartPreviousClose).unwrap_or(0.0),
                market_cap_approx: None,
                pe_ratio_approx: None,
                notes: format!("Currency: {}, Symbol: {}", m.currency.clone().unwrap_or_default(), m.symbol),
            }));
        }
        Ok(None)
    }
}
