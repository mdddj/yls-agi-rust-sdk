use yls_agi_rust_sdk::{GeminiClient, GeminiImageRequest, GeminiModel, GenerationOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GeminiClient::from_env()?;
    let response = client
        .generate_image(
            GeminiImageRequest::new(
                GeminiModel::Gemini3ProImagePreview,
                "像素风哥布林,站立,行走,攻击 3 种动画,每行 8 帧
",
            )
            .with_options(GenerationOptions {
                temperature: Some(1.0),
                max_tokens: Some(8192),
                ..Default::default()
            }),
        )
        .await?;

    for (index, image) in response.images.iter().enumerate() {
        let path = format!("gemini-image-{}.png", index + 1);
        image.save(&path)?;
        println!("saved {path}");
    }

    for text in response.text {
        println!("{text}");
    }

    Ok(())
}
