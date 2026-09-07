use anyhow::Result;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, InferenceConfiguration, Message,
};

const MODEL: &str = "eu.amzon.nova-lite-v1:0";

pub async fn explain_move(client: &Client, ticker: &str, dp: f64, headlines: &str)->Result<String>{
    let prompt = format!("{ticker} moved {dp:+.2}% today. Headlines from the last 24 hours:\n\
         {headlines}\n\n\
         Explain the move in one - two sentence, using only these headlines. \
         If they do not explain the move, answer exactly: No clear driver. \
         Never invent a cause.");

        let resp = client
            .converse()
            .model_id(MODEL)
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

    Ok(text.trim().to_string())

}