use std::{fs, path::PathBuf};
use yls_agi_rust_sdk::{
    ChatMessage, ChatRequest, ClientBuilder, GenerationOptions, ImageMimeType, OpenAiModel,
    Provider,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image_path = PathBuf::from("gemini-image-1.png");
    if !image_path.exists() {
        return Err(format!("image not found: {}", image_path.display()).into());
    }

    let image_bytes = fs::read(&image_path)?;
    let client = ClientBuilder::from_env()?.without_proxy().build()?;

    let request = ChatRequest::new(
        OpenAiModel::Gpt41,
        vec![
            ChatMessage::system("You are a concise vision assistant."),
            ChatMessage::user("这是什么？请用中文简短描述。")
                .with_image_bytes(ImageMimeType::Png, &image_bytes),
        ],
    )
    .with_options(GenerationOptions {
        temperature: Some(0.2),
        max_tokens: Some(256),
        ..Default::default()
    });

    let response = client.chat(Provider::OpenAi, request).await?;
    println!("{}", response.message.text_content());
    Ok(())
}
