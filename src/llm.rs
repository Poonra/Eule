use anyhow::Result;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, InferenceConfiguration, Message, SystemContentBlock,
};

const MODEL: &str = "eu.anthropic.claude-haiku-4-5-20251001-v1:0";

const SYSTEM: &str = "You explain a single-day stock price move, using only the material provided.

  Write one sentence naming the most likely driver, drawn from a specific headline. Add a
  second sentence only when the material contains a concrete figure, product, customer,
  ruling or guidance that sharpens it. Never a third.

  Never invent a cause, a figure, a date, a quarter or a fiscal year. If earnings figures are
  supplied they are today's report and are the presumed driver: cite actual versus estimate
  exactly as given, and do not name the quarter unless it is stated.

  A headline that plausibly accounts for the move is an explanation - use it. Reserve
  \"No clear driver.\" for when the material genuinely says nothing about the move; in that
  case reply with exactly that and nothing more.

  Reply with the sentence or sentences only: no preamble, no bullets, no restating of these rules.";

pub async fn explain_move(
    client: &Client,
    ticker: &str,
    dp: f64,
    headlines: &str,
    earnings: Option<&str>,
) -> Result<String> {
    let mut prompt = format!(
        "{ticker} moved {dp:+.2}% today.\n\nHeadlines from the last 24 hours:\n{headlines}"
    );
    if let Some(e) = earnings {
        prompt += &format!("\n\nThe company reported earnings today: {e}");
    }

    let resp = client
        .converse()
        .model_id(MODEL)
        .system(SystemContentBlock::Text(SYSTEM.to_string()))
        .messages(
            Message::builder()
                .role(ConversationRole::User)
                .content(ContentBlock::Text(prompt))
                .build()?,
        )
        .inference_config(InferenceConfiguration::builder().max_tokens(300).build())
        .send()
        .await?;

    let text = resp
        .output()
        .and_then(|o| o.as_message().ok())
        .and_then(|m| m.content().first())
        .and_then(|c| c.as_text().ok())
        .cloned()
        .unwrap_or_else(|| "No clear driver.".into());

    Ok(text.trim().trim_matches('"').trim().to_string())
}
