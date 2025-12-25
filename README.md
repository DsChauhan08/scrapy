# Scrapy: Financial Data Packetizer for LLMs

**Scrapy** is a high-performance command-line tool written in Rust designed to fetch, aggregate, and "packetize" real-time financial data into a clean, context-rich text format optimized for Large Language Models (LLMs) and AI Agents.

Stop parsing messy JSON or scraping generic HTML. WeekChart gives you a single, dense, semantic block of text containing price history, news, and insider activity, ready to be pasted into a prompt or ingested by an agent.

## 🚀 Key Features

*   **Real-Time Price Data**: Fetches 1-minute intraday bars (1-week window) directly from Yahoo Finance and resamples them into **1-Hour Regular Session Bars** (excluding pre/post market noise).
*   **Advanced News Scraping**:
    *   **Full Body Extraction**: Attempts to scrape the actual article text (First 2 + Last 1 paragraphs) from Google News links.
    *   **Robust Fallback**: If a paywall or blocker is detected, it automatically extracts and *sanitizes* the RSS description summary to ensure you never get empty results.
    *   **Plain Text Only**: All HTML tags, "click here" links, and cookie warnings are stripped. Use pure text.
*   **Insider & Institutional Intelligence**:
    *   **Strict Time Filtering**: Fetch Insider Transactions strictly from the last N days (default: 7) to spot immediate signals.
    *   **Top Holders**: Identify current major owners (e.g., Berkshire Hathaway, Vanguard) to understand institutional sentiment.
*   **Zero-Config Output**: Generates valid Markdown/text with strict delimiters (`<<<SECTION>>>`) for easy parsing by other software.

## 🛠️ Build & Install

Ensure you have [Rust](https://www.rust-lang.org/tools/install) installed.

```bash
# Clone and build optimized binary
cargo build --release

# The binary will be at:
./target/release/weekchart
```

## 📖 Usage

### 1. Interactive Mode (Best for Humans)
Simply run the tool. It will ask for a ticker and save the result to `{TICKER}_packet.txt`.

```bash
./target/release/weekchart
# > Enter Ticker: MSFT
# [Status] Fetching data...
# [Success] Packet saved to: MSFT_packet.txt
```

### 2. CLI / Pipeline Mode (Best for Software)
Pass arguments to pipe data directly into another application or LLM context window.

```bash
# Standard usage (Last 7 days of data)
./target/release/weekchart --ticker NVDA --output packet.txt

# Strict Insider Search (Last 2 days only)
./target/release/weekchart --ticker TSLA --window-days 2 --output tsla_latest.txt

# Pipe directly to stdout (silence logs)
./target/release/weekchart --ticker AMZN > data.txt
```

### Options
*   `--ticker <SYMBOL>`: Target stock symbol (e.g., AAPL).
*   `--window-days <N>`: Days of data to fetch (Default: 7). Also controls the lookback window for Insider Transactions.
*   `--no-news`: Skip news scraping (faster).
*   `--no-finance`: Skip financial snapshots.
*   `--output <FILE>`: Save output to specific file path.

## 📦 Output Format

The output is designed for **Machine Parsing**. It uses unique delimiters that are unlikely to collide with article text.

```text
<<<TICKER_PACKET_V1>>>
TICKER: MSFT
TZ: America/New_York
SESSION: REGULAR (09:30-16:00)
...

<<<PRICE_BARS_1H_CSV>>>
# ts_local, o, h, l, c, v
2025-12-24T09:30:00-05:00, 412.50, 415.20, 411.80, 414.10, 5200100
...
<<<END_PRICE_BARS_1H_CSV>>>

<<<NEWS_TOP10_BODY>>>
Thu, 25 Dec 2025 | Yahoo Finance | Microsoft AI Push...
(Summary): Microsoft shares rose slightly in after-hours trading as CEO Satya Nadella announced...
-------------------
...
<<<END_NEWS_TOP10_BODY>>>

<<<INSIDER_AND_INSTITUTIONAL_ACTIVITY>>>
--- RECENT INSIDER TRANSACTIONS (Last 7 Days) ---
2025-12-24 | NADELA SATYA | CEO | Sale | 50M

--- TOP INSTITUTIONAL & FUND HOLDERS ---
Vanguard Group, Inc. (The) | 8.97%
<<<END_INSIDER_AND_INSTITUTIONAL_ACTIVITY>>>
```

## 🔌 Integration Guide

To use this tool within your own software (e.g., Python, Node.js):

1.  **Run the binary** using `subprocess` or `exec`.
2.  **Capture stdout**.
3.  **Split by Delimiters**:
    *   Regex: `<<<NEWS_TOP10_BODY>>>\n([\s\S]*?)<<<END_NEWS_TOP10_BODY>>>`
    *   Regex: `<<<PRICE_BARS_1H_CSV>>>\n([\s\S]*?)<<<END_PRICE_BARS_1H_CSV>>>`

All text inside the News bodies is guaranteed to be sanitized (no raw HTML), making it safe to feed directly into RAG pipelines.
