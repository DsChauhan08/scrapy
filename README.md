# WeekChart - Stock Packetizer

A high-performance Rust tool to packetize intraday stock data for LLM consumption.

## Build
```bash
cargo build --release
```

## Run

### Interactive (Best for local testing)
```bash
./target/release/weekchart
```
Follow the prompts to enter a ticker. The tool will look for `{TICKER}.csv` in the current directory or `data/` folder.

### CLI (Best for pipelines)
```bash
./target/release/weekchart --ticker AMZN --source-path /path/to/AMZN.csv
```

## Options
- `--window-days <N>`: Number of recent trading days to include (default: 7).
- `--no-news`: Skip news section.
- `--no-senate`: Skip senate activity.
- `--no-finance`: Skip finance snapshot.

## Data Format
Input CSV must contain minute bars with headers: `ts,o,h,l,c,v`.
Timestamp `ts` must be RFC3339 UTC (e.g., `2025-12-20T15:31:00Z`).

## Architecture
- `src/main.rs`: Entrypoint.
- `src/market.rs`: Resampling logic (1-minute -> 1-hour, Regular US Session).
- `src/collectors.rs`: Interfaces for external data (News, Senate, Finance).
