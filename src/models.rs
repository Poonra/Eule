use askama::Template;
use serde::Deserialize;

pub struct Stock {
    pub ticker: String,
    pub quote: Quote,
    pub next_earnings: Option<String>,
    pub news: Vec<NewsItem>,
    pub explanation: String,
    pub last_earnings: Option<String>,
}
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Quote {
    pub c: f64,  //current price
    pub d: f64,  //change
    pub dp: f64, //change %
    pub pc: f64, //previous close
    pub t: i64,  //quote timestamp
}

#[derive(Deserialize)]
pub struct Watchlist {
    pub tickers: Vec<String>,
}

pub struct Row {
    pub ticker: String,
    pub price: String,
    pub change: String,
    pub next_earnings: String,
    pub explanation: String,
    pub news: Vec<NewsItem>,
    pub last_earnings: String,
}

#[derive(Template)]
#[template(path = "briefing.html")]
pub struct Briefing {
    pub date: String,
    pub rows: Vec<Row>,
}

#[derive(Deserialize)]
pub struct EarningsCalendar {
    #[serde(rename = "earningsCalendar")]
    pub event: Vec<EarningsEvent>,
}

#[derive(Deserialize)]
pub struct EarningsEvent {
    pub date: String,
    #[serde(rename = "epsEstimate")]
    pub eps_estimate: Option<f64>,
    #[serde(rename = "epsActual")]
    pub eps_actual: Option<f64>,
    #[serde(rename = "revenueEstimate")]
    pub revenue_estimate: Option<f64>,
    #[serde(rename = "revenueActual")]
    pub revenue_actual: Option<f64>,
}
#[derive(Deserialize, Clone)]
pub struct NewsItem {
    pub headline: String,
    pub summary: String,
    pub source: String,
    pub url: String,
}
