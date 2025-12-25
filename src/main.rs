use anyhow::{Context, Result};
use clap::Parser;
use chrono::{DateTime, Utc};
use csv::StringRecord;
use std::io::{self, Write};
use std::path::Path;

mod market;
mod collectors;

use market::{MinuteBar, resample_1h_regular_session};
use collectors::{NewsCollector, SenateCollector, FinanceSnapshotCollector};
use collectors::{NullNewsCollector, NullSenateCollector, StubFinanceSnapshotCollector};

#[derive(Parser)]
struct Args {
    /// Ticker symbol (e.g., AAPL)
    #[arg(long)]
    ticker: Option<String>,

    /// Path to the CSV file containing minute bars
    #[arg(long("source-path"))]
    source_path: Option<String>,

    /// Number of trading days to include
    #[arg(long, default_value = "7")]
    window_days: i64,

    /// Disable news section
    #[arg(long)]
    no_news: bool,

    /// Disable senate activity section
    #[arg(long)]
    no_senate: bool,

    /// Disable finance snapshot section
    #[arg(long)]
    no_finance: bool,
}

fn parse_row(rec: &StringRecord) -> Result<MinuteBar> {
    // Expected: ts, o, h, l, c, v
    let ts_str = rec.get(0).context("missing ts")?;
    let ts: DateTime<Utc> = ts_str.parse().context("bad ts format")?;
    
    let o: f64 = rec.get(1).context("missing o")?.parse().context("bad o")?;
    let h: f64 = rec.get(2).context("missing h")?.parse().context("bad h")?;
    let l: f64 = rec.get(3).context("missing l")?.parse().context("bad l")?;
    let c: f64 = rec.get(4).context("missing c")?.parse().context("bad c")?;
    let v: u64 = rec.get(5).context("missing v")?.parse().context("bad v")?;
    
    Ok(MinuteBar { ts_utc: ts, o, h, l, c, v })
}

fn prompt_input(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    // Interactive Mode Logic
    let ticker = match args.ticker {
        Some(t) => t.to_uppercase(),
        None => {
            let t = prompt_input("Enter Ticker (e.g. AMZN): ")?;
            if t.is_empty() {
                anyhow::bail!("Ticker cannot be empty");
            }
            t.to_uppercase()
        }
    };

    let source_path = match args.source_path {
        Some(p) => p,
        None => {
            // Check default locations
            let default_name = format!("{}.csv", ticker);
            let candidates = vec![
                default_name.clone(),
                format!("data/{}", default_name),
                format!("sample_data/{}", default_name),
            ];
            
            let found = candidates.iter().find(|p| Path::new(p).exists());
            
            if let Some(p) = found {
                println!("Found data at: {}", p);
                p.clone()
            } else {
                let p = prompt_input(&format!("Enter path to CSV for {} [default: ./{}]: ", ticker, default_name))?;
                if p.is_empty() {
                    default_name
                } else {
                    p
                }
            }
        }
    };

    // 1. Load Price Data
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(&source_path)
        .with_context(|| format!("failed to open csv {}", source_path))?;

    let mut rows: Vec<MinuteBar> = Vec::with_capacity(50_000);
    for r in rdr.records() {
        let rec = r?;
        rows.push(parse_row(&rec)?);
    }
    // Sort logic just in case CSV isn't perfectly sorted
    rows.sort_by_key(|b| b.ts_utc);

    // 2. Resample
    let chart = resample_1h_regular_session(&ticker, &rows, args.window_days);

    // 3. Collect Extra Data (Stubs)
    let news_lines = if !args.no_news {
        let col = NullNewsCollector;
        let items = col.collect_news(&ticker, args.window_days)?;
        if items.is_empty() {
            String::new()
        } else {
             items.iter().map(|item| {
                 format!("{} | {} | {} | {}", item.datetime, item.source, item.headline, item.url)
             }).collect::<Vec<_>>().join("\n")
        }
    } else {
        String::new()
    };

    let senate_lines = if !args.no_senate {
        let col = NullSenateCollector;
        let items = col.collect_senate_activity(&ticker, args.window_days)?;
         if items.is_empty() {
            String::new()
        } else {
             items.iter().map(|item| {
                 format!("{} | {} | {} | {} | {}", item.date, item.chamber, item.member_name, item.activity_type, item.notes.as_deref().unwrap_or(""))
             }).collect::<Vec<_>>().join("\n")
        }
    } else {
        String::new()
    };

    let finance_block = if !args.no_finance {
        let col = StubFinanceSnapshotCollector;
        let snap = col.collect_snapshot(&ticker)?;
        if let Some(s) = snap {
            format!(
                "source: {}\nasof_utc: {}\nprice_last: {}\nmarket_cap_approx: {}\npe_ratio_approx: {}\nnotes: \"{}\"\n",
                s.source, s.asof_utc, s.price_last, 
                s.market_cap_approx.map(|v| v.to_string()).unwrap_or_default(),
                s.pe_ratio_approx.map(|v| v.to_string()).unwrap_or_default(),
                s.notes
            )
        } else {
             String::new()
        }
    } else {
        String::new()
    };


    // 4. Output Packet
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    writeln!(handle, "<<<TICKER_PACKET_V1>>>")?;
    writeln!(handle, "TICKER: {}", ticker)?;
    writeln!(handle, "TZ: America/New_York")?;
    writeln!(handle, "SESSION: REGULAR (09:30-16:00)")?;
    writeln!(handle, "WINDOW_DAYS: {}", args.window_days)?;
    writeln!(handle, "BAR_SIZE: 1h")?;
    writeln!(handle, "BARS_COUNT: {}", chart.bars.len())?;
    writeln!(handle)?;

    writeln!(handle, "<<<PRICE_BARS_1H_CSV>>>")?;
    writeln!(handle, "# ts_local,o,h,l,c,v")?;
    for b in &chart.bars {
        writeln!(handle, "{},{:.6},{:.6},{:.6},{:.6},{}", b.ts_local, b.o, b.h, b.l, b.c, b.v)?;
    }
    writeln!(handle, "<<<END_PRICE_BARS_1H_CSV>>>")?;
    writeln!(handle)?;

    writeln!(handle, "<<<NEWS_TOP10_1W>>>")?;
    writeln!(handle, "# Each line: datetime | source | headline | url")?;
    if !news_lines.is_empty() {
        writeln!(handle, "{}", news_lines)?;
    }
    writeln!(handle, "<<<END_NEWS_TOP10_1W>>>")?;
    writeln!(handle)?;

    writeln!(handle, "<<<SENATE_ACTIVITY>>>")?;
    writeln!(handle, "# Each line: date | chamber | member_name | activity_type | notes")?;
     if !senate_lines.is_empty() {
        writeln!(handle, "{}", senate_lines)?;
    }
    writeln!(handle, "<<<END_SENATE_ACTIVITY>>>")?;
    writeln!(handle)?;

    writeln!(handle, "<<<FINANCE_SNAPSHOT>>>")?;
    if !finance_block.is_empty() {
        write!(handle, "{}", finance_block)?;
    }
    writeln!(handle, "<<<END_FINANCE_SNAPSHOT>>>")?;
    writeln!(handle)?;

    writeln!(handle, "<<<NOTES>>>")?;
    writeln!(handle, "- This packet is plain text designed for a 3B LLM and downstream ML models.")?;
    writeln!(handle, "- Parsing is simplified by strong delimiters (<<<...>>>).")?;
    writeln!(handle, "- Bars are for regular US trading sessions only; final bar per day may be shorter.")?;
    writeln!(handle, "- Data quality / licensing for intraday prices and news is handled separately upstream.")?;
    writeln!(handle, "<<<END_NOTES>>>")?;
    writeln!(handle, "<<<END_TICKER_PACKET_V1>>>")?;

    Ok(())
}
