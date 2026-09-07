use anyhow::{Context, Result};
use askama::Template;
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

struct Row {
    ticker: String,
    price: String,
    change: String,
    next_earnings: String,
}

#[derive(Template)]
#[template(path = "briefing.html")]
struct Briefing {
    date: String,
    rows: Vec<Row>,
}

#[derive(Deserialize)]
struct EarningsCalendar {
    #[serde(rename = "earningsCalendar")]
    event: Vec<EarningsEvent>,
}

#[derive(Deserialize)]
struct EarningsEvent {
    date: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let key = std::env::var("FINNHUB_KEY").context("FINNHUB_KEY not set")?;

    let watchlist: Watchlist = toml::from_str(include_str!("../watchlist.toml"))?;

    let mut rows: Vec<(String, Quote, Option<String>)> = Vec::new();

    for ticker in &watchlist.tickers {
        let url = format!("https://finnhub.io/api/v1/quote?symbol={ticker}&token={key}");
        let quote: Quote = reqwest::get(&url).await?.json().await?;

        let today = chrono::Local::now();
        let horizon = today + chrono::Duration::days(90);
        let cal_url = format!(
            "https://finnhub.io/api/v1/calendar/earnings?from={}&to={}&symbol={ticker}&token={key}",
            today.format("%Y-%m-%d"),
            horizon.format("%Y-%m-%d")
        );
        let cal: EarningsCalendar = reqwest::get(&cal_url).await?.json().await?;
        let next = cal.event.iter().map(|e| &e.date).min().cloned();

        rows.push((ticker.clone(), quote, next));
    }
    rows.sort_by(|a, b| b.1.dp.abs().total_cmp(&a.1.dp.abs()));

    let briefing = Briefing {
        date: chrono::Local::now().format("%Y-%m-%d").to_string(),
        rows: rows
            .iter()
            .map(|(ticker, q, next)| Row {
                ticker: ticker.clone(),
                price: format!("{:.2}", q.c),
                change: format!("{:.2}", q.dp),
                next_earnings: next.clone().unwrap_or_else(|| "—".into()),
            })
            .collect(),
    };

    std::fs::create_dir_all("out")?;
    std::fs::write("out/index.html", briefing.render()?)?;
    println!("write out/index.html");

    Ok(())
}
