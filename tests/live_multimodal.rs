use std::{fs, path::PathBuf};
use yls_agi_rust_sdk::{
    ChatMessage, ChatRequest, ClaudeModel, ClientBuilder, GeminiModel, GenerationOptions,
    ImageMimeType, OpenAiModel, Provider,
};

#[tokio::test]
#[ignore = "live network test that requires YLS_AGI_KEY and gemini-image-1.png"]
async fn gemini_can_describe_local_image() -> Result<(), Box<dyn std::error::Error>> {
    let image_path = PathBuf::from("gemini-image-1.png");
    let image_bytes = fs::read(&image_path)?;

    let client = ClientBuilder::from_env()?.without_proxy().build()?;
    let request = ChatRequest::new(
        GeminiModel::Gemini3ProPreview,
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

    let response = client.chat(Provider::Gemini, request).await?;
    let text = response.message.text_content();
    assert!(
        !text.trim().is_empty(),
        "gemini returned an empty description"
    );
    println!("{text}");
    Ok(())
}

#[tokio::test]
#[ignore = "live network test that requires YLS_AGI_KEY and gemini-image-1.png"]
async fn openai_can_describe_local_image() -> Result<(), Box<dyn std::error::Error>> {
    let image_path = PathBuf::from("gemini-image-1.png");
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
    let text = response.message.text_content();
    assert!(
        !text.trim().is_empty(),
        "openai returned an empty description"
    );
    println!("{text}");
    Ok(())
}

#[tokio::test]
#[ignore = "live network test that requires YLS_AGI_KEY and gemini-image-1.png"]
async fn claude_can_describe_local_image() -> Result<(), Box<dyn std::error::Error>> {
    let image_path = PathBuf::from("gemini-image-1.png");
    let image_bytes = fs::read(&image_path)?;

    let client = ClientBuilder::from_env()?.without_proxy().build()?;
    let request = ChatRequest::new(
        ClaudeModel::ClaudeSonnet4520250929,
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

    let response = client.chat(Provider::Claude, request).await?;
    let text = response.message.text_content();
    assert!(
        !text.trim().is_empty(),
        "claude returned an empty description"
    );
    println!("{text}");
    Ok(())
}
