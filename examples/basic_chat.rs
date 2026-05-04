use yls_agi_rust_sdk::{
    ChatMessage, ChatRequest, ClientBuilder, GeminiModel, GenerationOptions, Provider,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ClientBuilder::from_env()?.without_proxy().build()?;

    let request = ChatRequest::new(
        GeminiModel::Gemini3ProPreview,
        vec![
            ChatMessage::system("You are a concise assistant."),
            ChatMessage::user("用一句话介绍伊莉思"),
        ],
    )
    .with_options(GenerationOptions {
        temperature: Some(0.2),
        max_tokens: Some(256),
        ..Default::default()
    });

    let response = client.chat(Provider::Gemini, request).await?;
    println!("{}", response.message.content);
    Ok(())
}
