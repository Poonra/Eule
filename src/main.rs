mod llm;

use anyhow::{Context, Result};
use askama::Template;
use serde::Deserialize;
struct Stock {
    ticker: String,
    quote: Quote,
    next_earnings: Option<String>,
    news: Vec<NewsItem>,
    explanation: String,
}
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
    explanation: String,
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
#[derive(Deserialize)]
struct NewsItem {
    headline: String,
    summary: String,
    source: String,
    url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let key = std::env::var("FINNHUB_KEY").context("FINNHUB_KEY not set")?;

    let watchlist: Watchlist = toml::from_str(include_str!("../watchlist.toml"))?;

    let mut stocks: Vec<Stock> = Vec::new();

    let aws = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let bedrock = aws_sdk_bedrockruntime::Client::new(&aws);

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

        let yesterday = today-chrono::Duration::days(1);
        let news_url = format!("https://finnhub.io/api/v1/company-news?symbol={ticker}&from={}&to={}&token={key}",
                               yesterday.format("%Y-%m-%d"),
                               today.format("%Y-%m-%d")
        );

        let news: Vec<NewsItem> = reqwest::get(&news_url).await?.json().await?;
        let news: Vec<NewsItem> = news.into_iter().take(5).collect();


        let headlines = news
        .iter()
            .map(|n| format!("- {} ({}): {}", n.headline, n.source, n.summary))
            .collect::<Vec<_>>()
            .join("\n");

        let explanation = if news.is_empty() {
            "no clear driver".to_string()
        } else {
            llm::explain_move(&bedrock, ticker, quote.dp, &headlines).await?
        };

        stocks.push(Stock {
            ticker:ticker.clone(),
            quote,
            next_earnings:next,
            news,
            explanation,
        });


    }
    stocks.sort_by(|a, b| b.quote.dp.abs().total_cmp(&a.quote.dp.abs()));

    let briefing = Briefing {
        date: chrono::Local::now().format("%Y-%m-%d").to_string(),
        rows: stocks
            .iter()
            .map(|s| Row {
                ticker: s.ticker.clone(),
                price: format!("{:.2}", s.quote.c),
                change: format!("{:.2}", s.quote.dp),
                next_earnings: s.next_earnings.clone().unwrap_or_else(|| "—".into()),
                explanation: s.explanation.clone(),
            })
            .collect(),
    };

    std::fs::create_dir_all("out")?;
    std::fs::write("out/index.html", briefing.render()?)?;
    println!("write out/index.html");

    Ok(())
}
