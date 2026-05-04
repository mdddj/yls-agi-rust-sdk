use yls_agi_rust_sdk::{
    AuthMode, ChatMessage, ChatRequest, ClientBuilder, GenerationOptions, OpenAiModel, Provider,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ClientBuilder::from_env()?
        .with_claude_auth_mode(AuthMode::AuthorizationKey)
        .build()?;

    let request = ChatRequest::new(
        OpenAiModel::Gpt41,
        vec![
            ChatMessage::system("You are a concise assistant."),
            ChatMessage::user("用一句话介绍伊莉思 SDK。"),
        ],
    )
    .with_options(GenerationOptions {
        temperature: Some(0.2),
        max_tokens: Some(256),
        ..Default::default()
    });

    let response = client.chat(Provider::OpenAi, request).await?;
    println!("{}", response.message.content);
    Ok(())
}
