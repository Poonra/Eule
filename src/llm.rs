use anyhow::Result;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, InferenceConfiguration, Message, SystemContentBlock,
};

const MODEL: &str = "eu.anthropic.claude-haiku-4-5-20251001-v1:0";

const SYSTEM: &str = "You explain single-day stock price moves in one sentence,
using only the headlines provided. Never invent a cause. Reply with the sentence only,
no preamble and no restating of these rules.
If the headlines do not explain the move, reply with exactly this and nothing more:
\"No clear driver.\"";

pub async fn explain_move(
    client: &Client,
    ticker: &str,
    dp: f64,
    headlines: &str,
) -> Result<String> {
    let prompt = format!(
        "{ticker} moved {dp:+.2}% today.\n\nHeadlines from the last 24 hours:\n{headlines}"
    );

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
        .inference_config(InferenceConfiguration::builder().max_tokens(100).build())
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
