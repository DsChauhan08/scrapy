use anyhow::{Context, Result};
use clap::Parser;
use std::io::{self, Write};
use std::fs::File;

mod market;
mod collectors;
mod fetcher; 

use market::resample_1h_regular_session;
use collectors::{NewsCollector, InsiderCollector, FinanceSnapshotCollector}; 
use collectors::{GoogleNewsCollector, YahooInsiderCollector, YahooSnapshotCollector}; 

#[derive(Parser)]
struct Args {
    #[arg(long)]
    ticker: Option<String>,

    #[arg(long, default_value = "7")]
    window_days: i64,

    #[arg(long)]
    no_news: bool,

    #[arg(long)]
    no_senate: bool, 

    #[arg(long)]
    no_finance: bool,
    
    #[arg(long)]
    output: Option<String>,
}

fn prompt_input(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

fn main() -> Result<()> {
    let args_cli = Args::parse();
    let is_interactive = args_cli.ticker.is_none();
    
    // Interactive Mode Logic
    let ticker = match args_cli.ticker {
        Some(t) => t.to_uppercase(),
        None => {
            let t = prompt_input("Enter Ticker (e.g. AMZN): ")?;
            if t.is_empty() {
                anyhow::bail!("Ticker cannot be empty");
            }
            t.to_uppercase()
        }
    };

    if is_interactive {
        eprintln!("Fetching data for {} from the internet...", ticker);
        eprintln!("(This may take a few seconds to scrape news bodies and insider info)");
    }

    let (rows, meta) = fetcher::fetch_minute_bars(&ticker, args_cli.window_days)
        .with_context(|| format!("Failed to fetch price data for {}", ticker))?;
    
    let chart = resample_1h_regular_session(&ticker, &rows, args_cli.window_days);

    // 3. Collect Extra Data (Live!)
    let news_block = if !args_cli.no_news {
        let col = GoogleNewsCollector;
        match col.collect_news(&ticker, args_cli.window_days) {
            Ok(items) => {
                if items.is_empty() {
                    "No recent news found.".to_string()
                } else {
                     items.iter().take(10).map(|item| {
                         format!("{} | {} | {}\n{}\n-------------------", 
                            item.datetime, item.source, item.headline, item.content_snippet)
                     }).collect::<Vec<_>>().join("\n")
                }
            }
            Err(e) => format!("Error fetching news: {}", e)
        }
    } else {
        String::new()
    };

    let insider_block = if !args_cli.no_senate { 
        let col = YahooInsiderCollector;
        // Pass the window_days for strict filtering!
        match col.collect_activity(&ticker, args_cli.window_days) {
            Ok((trades, holders)) => {
                let mut s = String::new();
                if trades.is_empty() {
                    s.push_str(&format!("--- RECENT INSIDER TRANSACTIONS (Last {} Days) ---\n", args_cli.window_days));
                    s.push_str("No transactions found in this period.\n");
                } else {
                    s.push_str(&format!("--- RECENT INSIDER TRANSACTIONS (Last {} Days) ---\n", args_cli.window_days));
                    s.push_str("# Date | Entity | Relation | Type | Value\n");
                    for t in trades {
                        s.push_str(&format!("{} | {} | {} | {} | {}\n", t.date, t.entity_name, t.relation, t.transaction_type, t.value_approx));
                    }
                }
                
                s.push_str("\n--- TOP INSTITUTIONAL & FUND HOLDERS ---\n");
                s.push_str("# Holder | % Held\n");
                for h in holders {
                     s.push_str(&format!("{} | {}\n", h.holder_name, h.pct_held));
                }
                s
            },
            Err(e) => format!("Error fetching insider info: {}", e)
        }
    } else {
        String::new()
    };

    let finance_block = if !args_cli.no_finance {
        let col = YahooSnapshotCollector;
        match col.collect_snapshot(&ticker, meta.as_ref()) {
            Ok(Some(s)) => {
                format!(
                    "source: {}\nasof_utc: {}\nprice_last: {}\nnotes: \"{}\"\n",
                    s.source, s.asof_utc, s.price_last, s.notes
                )
            },
            Ok(None) => "No snapshot available.".to_string(),
            Err(e) => format!("Error fetching snapshot: {}", e)
        }
    } else {
        String::new()
    };


    // 4. Build Packet String
    let mut packet = String::new();
    packet.push_str("<<<TICKER_PACKET_V1>>>\n");
    packet.push_str(&format!("TICKER: {}\n", ticker));
    packet.push_str("TZ: America/New_York\n");
    packet.push_str("SESSION: REGULAR (09:30-16:00)\n");
    packet.push_str(&format!("WINDOW_DAYS: {}\n", args_cli.window_days));
    packet.push_str("BAR_SIZE: 1h\n");
    packet.push_str(&format!("BARS_COUNT: {}\n", chart.bars.len()));
    packet.push_str("\n");

    packet.push_str("<<<PRICE_BARS_1H_CSV>>>\n");
    packet.push_str("# ts_local,o,h,l,c,v\n");
    for b in &chart.bars {
        packet.push_str(&format!("{},{:.6},{:.6},{:.6},{:.6},{}\n", b.ts_local, b.o, b.h, b.l, b.c, b.v));
    }
    packet.push_str("<<<END_PRICE_BARS_1H_CSV>>>\n");
    packet.push_str("\n");

    packet.push_str("<<<NEWS_TOP10_BODY>>>\n");
    if !news_block.is_empty() {
        packet.push_str(&news_block);
        packet.push_str("\n");
    }
    packet.push_str("<<<END_NEWS_TOP10_BODY>>>\n");
    packet.push_str("\n");

    packet.push_str("<<<INSIDER_AND_INSTITUTIONAL_ACTIVITY>>>\n");
     if !insider_block.is_empty() {
        packet.push_str(&insider_block);
        packet.push_str("\n");
    }
    packet.push_str("<<<END_INSIDER_AND_INSTITUTIONAL_ACTIVITY>>>\n");
    packet.push_str("\n");

    packet.push_str("<<<FINANCE_SNAPSHOT>>>\n");
    if !finance_block.is_empty() {
        packet.push_str(&finance_block);
    }
    packet.push_str("<<<END_FINANCE_SNAPSHOT>>>\n");
    packet.push_str("\n");

    // 5. Output Handling
    print!("{}", packet);

    let output_file = if let Some(path) = args_cli.output {
        Some(path)
    } else if is_interactive {
        Some(format!("{}_packet.txt", ticker))
    } else {
        None
    };

    if let Some(path) = output_file {
        let mut f = File::create(&path).with_context(|| format!("failed to create output file {}", path))?;
        f.write_all(packet.as_bytes())?;
        if is_interactive {
            eprintln!("Packet saved to: {}", path);
        }
    }

    Ok(())
}
