mod llm;

use anyhow::{Context, Result};
use askama::Template;
use aws_sdk_s3::primitives::ByteStream;
use serde::Deserialize;
struct Stock {
    ticker: String,
    quote: Quote,
    next_earnings: Option<String>,
    news: Vec<NewsItem>,
    explanation: String,
    last_earnings: Option<String>,
}
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Quote {
    c: f64,   //current price
    d: f64,  //change
    dp: f64,  //change %
    pc: f64, //previous close
    t: i64,   //quote timestamp
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
    news: Vec<NewsItem>,
    last_earnings: String,
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
    #[serde(rename = "epsEstimate")]
    eps_estimate: Option<f64>,
    #[serde(rename = "epsActual")]
    eps_actual: Option<f64>,
    #[serde(rename = "revenueEstimate")]
    revenue_estimate: Option<f64>,
    #[serde(rename = "revenueActual")]
    revenue_actual: Option<f64>,
}
#[derive(Deserialize, Clone)]
struct NewsItem {
    headline: String,
    summary: String,
    source: String,
    url: String,
}
#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    if std::env::var("AWS_LAMBDA_RUNTIME_API").is_ok() {
        lambda_runtime::run(lambda_runtime::service_fn(handler))
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    } else {
        run_briefing().await
    }
}

async fn handler(
    _event: lambda_runtime::LambdaEvent<serde_json::Value>,
) -> Result<(), lambda_runtime::Error> {
    run_briefing().await.map_err(|e| e.into())
}
async fn run_briefing() -> Result<()> {
    let key = std::env::var("FINNHUB_KEY").context("FINNHUB_KEY not set")?;

    let mut stocks: Vec<Stock> = Vec::new();

    let aws = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let bedrock = aws_sdk_bedrockruntime::Client::new(&aws);

    let watchlist = load_watchlist(&aws).await?;

    for ticker in &watchlist.tickers {
        let url = format!("https://finnhub.io/api/v1/quote?symbol={ticker}&token={key}");
        let quote: Quote = reqwest::get(&url).await?.json().await?;

        let quote_day = chrono::DateTime::from_timestamp(quote.t, 0)
            .context("bad quote timestamp")?
            .date_naive();
        let session = quote_day.to_string();
        let cal_url = format!(
            "https://finnhub.io/api/v1/calendar/earnings?from={}&to={}&symbol={ticker}&token={key}",
            quote_day - chrono::Duration::days(90),
            quote_day + chrono::Duration::days(90)
        );
        let cal: EarningsCalendar = reqwest::get(&cal_url).await?.json().await?;
        let next = cal
            .event
            .iter()
            .filter(|e| e.date >= session)
            .map(|e| e.date.clone())
            .min();

        let last_earnings = cal
            .event
            .iter()
            .filter(|e| e.eps_actual.is_some())
            .max_by(|a, b| a.date.cmp(&b.date))
            .and_then(last_earnings_line);

        let news_url = format!(
            "https://finnhub.io/api/v1/company-news?symbol={ticker}&from={quote_day}&to={quote_day}&token={key}"
        );

        let news: Vec<NewsItem> = reqwest::get(&news_url).await?.json().await?;
        let news: Vec<NewsItem> = news.into_iter().take(5).collect();

        let headlines = news
            .iter()
            .map(|n| format!("- {} ({}): {}", n.headline, n.source, n.summary))
            .collect::<Vec<_>>()
            .join("\n");

        let reported_today = cal
            .event
            .iter()
            .find(|e| e.date == session)
            .and_then(last_earnings_line);

        let explanation = if news.is_empty() {
            "No clear driver.".to_string()
        } else {
            llm::explain_move(&bedrock, ticker, quote.dp, &headlines, reported_today.as_deref()).await?
        };

        stocks.push(Stock {
            ticker: ticker.clone(),
            quote,
            next_earnings: next,
            news,
            explanation,
            last_earnings,
        });
    }

    let session_day = stocks
        .first()
        .and_then(|s| chrono::DateTime::from_timestamp(s.quote.t, 0))
        .map(|d| d.date_naive())
        .context("no quotes fetched")?;

    stocks.sort_by(|a, b| b.quote.dp.abs().total_cmp(&a.quote.dp.abs()));

    let briefing = Briefing {
        date: session_day.to_string(),
        rows: stocks
            .iter()
            .map(|s| Row {
                ticker: s.ticker.clone(),
                price: format!("{:.2}", s.quote.c),
                change: format!("{:+.2}%", s.quote.dp),
                next_earnings: s.next_earnings.clone().unwrap_or_else(|| "—".into()),
                explanation: s.explanation.clone(),
                news: s.news.clone(),
                last_earnings: s.last_earnings.clone().unwrap_or_default(),
            })
            .collect(),
    };

    let html = briefing.render()?;

    match std::env::var("OUTPUT_TARGET").as_deref() {
        Ok("s3") => {
            let bucket = std::env::var("S3_BUCKET").context("S3_BUCKET not set")?;
            let s3 = aws_sdk_s3::Client::new(&aws);
            for key in ["index.html".to_string(), format!("{}.html", briefing.date)] {
                s3.put_object()
                    .bucket(&bucket)
                    .key(&key)
                    .body(ByteStream::from(html.clone().into_bytes()))
                    .content_type("text/html; charset=utf-8")
                    .send()
                    .await?;
            }
            println!("wrote s3://{bucket}/index.html");
        }
        _ => {
            std::fs::create_dir_all("out")?;
            std::fs::write("out/index.html", &html)?;
            println!("wrote out/index.html");
        }
    }

    Ok(())
}

async fn load_watchlist(aws: &aws_config::SdkConfig) -> Result<Watchlist> {
    let text = match std::env::var("OUTPUT_TARGET").as_deref() {
        Ok("s3") => {
            let bucket = std::env::var("S3_BUCKET").context("S3_BUCKET not set")?;
            let s3 = aws_sdk_s3::Client::new(aws);
            let obj = s3
                .get_object()
                .bucket(&bucket)
                .key("watchlist.toml")
                .send()
                .await
                .context("reading watchlist.toml from s")?;
            String::from_utf8(obj.body.collect().await?.to_vec())?
        }
        _ => std::fs::read_to_string("watchlist.toml")?,
    };
    Ok(toml::from_str(&text)?)
}

fn money(v: f64) -> String {
    if v.abs() >= 1e9 {
        format!("${:.1}B", v / 1e9)
    } else if v.abs() >= 1e6 {
        format!("${:.1}M", v / 1e6)
    } else {
        format!("${v:.0}")
    }
}

fn last_earnings_line(e: &EarningsEvent) -> Option<String> {
    let actual = e.eps_actual?;
    let estimate = e.eps_estimate?;

    let verdict = match actual.partial_cmp(&estimate)? {
        std::cmp::Ordering::Greater => "beat",
        std::cmp::Ordering::Less => "miss",
        std::cmp::Ordering::Equal => "in line",
    };

    let mut s = format!(
        "{} {verdict} · EPS ${actual:.2} vs ${estimate:.2} est",
        e.date
    );
    if let (Some(a), Some(est)) = (e.revenue_actual, e.revenue_estimate) {
        s += &format!(" · rev {} vs {} est", money(a), money(est));
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(date: &str, actual: Option<f64>, estimate: Option<f64>) -> EarningsEvent {
        EarningsEvent {
            date: date.into(),
            eps_actual: actual,
            eps_estimate: estimate,
            revenue_actual: None,
            revenue_estimate: None,
        }
    }

    #[test]
    fn next_earnings_ignores_the_past_quarter() {
        // the real NVDA shape: one reported quarter behind, one scheduled ahead
        let events = [
            ev("2026-11-17", None, Some(2.4659)),
            ev("2026-08-26", Some(2.22), Some(2.1384)),
        ];
        let session = "2026-09-11".to_string();

        let next = events
            .iter()
            .filter(|e| e.date >= session)
            .map(|e| e.date.clone())
            .min();
        assert_eq!(next.as_deref(), Some("2026-11-17"));

        let last = events
            .iter()
            .filter(|e| e.eps_actual.is_some())
            .max_by(|a, b| a.date.cmp(&b.date))
            .unwrap();
        assert_eq!(last.date, "2026-08-26");
        assert!(last_earnings_line(last).unwrap().contains("beat"));
    }

    #[test]
    fn no_line_until_the_actual_is_published() {
        assert!(last_earnings_line(&ev("2026-11-17", None, Some(2.46))).is_none());
    }
}
