use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Quote {
    c: f64,  //current price
    d: f64,  //change
    dp: f64, //change %
    pc: f64, //previous close
    t: i64,  //quote timestamp
}

#[derive(Deserialize)]
struct Watchlist {
    tickers: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let key = std::env::var("FINNHUB_KEY").context("FINNHUB_KEY not set")?;

    let watchlist: Watchlist = toml::from_str(include_str!("../watchlist.toml"))?;

    let mut rows: Vec<(String, Quote)> = Vec::new();

    for ticker in &watchlist.tickers {
        let url = format!("https://finnhub.io/api/v1/quote?symbol={ticker}&token={key}");
        let quote: Quote = reqwest::get(&url).await?.json().await?;
        rows.push((ticker.clone(), quote));
    }
    rows.sort_by(|a, b| b.1.dp.abs().total_cmp(&a.1.dp.abs()));

    for (ticker, quote) in &rows {
        println!("{ticker:<5} {:>8.2} {:>+7.2}%", quote.c, quote.dp);
    }

    Ok(())
}
