use std::{fs, path::PathBuf};
use yls_agi_rust_sdk::{
    GeminiClient, GeminiImageRequest, GeminiModel, GenerationOptions, ImageMimeType,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image_path = PathBuf::from("gemini-image-1.png");
    if !image_path.exists() {
        return Err(format!("image not found: {}", image_path.display()).into());
    }

    let image_bytes = fs::read(&image_path)?;
    let client = GeminiClient::from_env()?;
    let response = client
        .generate_image(
            GeminiImageRequest::new(
                GeminiModel::Gemini3ProImagePreview,
                "把这张角色图重绘成像素风游戏角色立绘，保留主体造型特征，透明背景。",
            )
            .with_reference_image_bytes(ImageMimeType::Png, &image_bytes)
            .with_options(GenerationOptions {
                temperature: Some(0.8),
                max_tokens: Some(8192),
                ..Default::default()
            }),
        )
        .await?;

    for (index, image) in response.images.iter().enumerate() {
        let path = format!("gemini-edited-image-{}.png", index + 1);
        image.save(&path)?;
        println!("saved {path}");
    }

    for text in response.text {
        println!("{text}");
    }

    Ok(())
}
