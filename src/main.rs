use std::fs;
use anyhow::{Context, Result};
use serde::Deserialize;
use askama::Template;

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

struct Row {
    ticker: String,
    price: String,
    change: String,
}

#[derive(Template)]
#[template(path = "briefing.html")]
struct Briefing {
    date: String,
    rows: Vec<Row>,
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

    let briefing = Briefing {
        date: chrono::Local::now().format("%Y-%m-%d").to_string(),
        rows: rows
            .iter()
            .map(|(ticker, q)| Row {
                ticker: ticker.clone(),
                price: format!("{:.2}", q.c),
                change: format!("{:.2}", q.dp),
            })
        .collect(),
    };

    std::fs::create_dir_all("out")?;
    std::fs::write("out/index.html", briefing.render()?)?;
    println!("write out/index.html");

    Ok(())
}
