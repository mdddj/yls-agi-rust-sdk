use yls_agi_rust_sdk::{GeminiClient, GeminiImageRequest, GeminiModel, GenerationOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GeminiClient::from_env()?;
    let response = client
        .generate_image(
            GeminiImageRequest::new(
                GeminiModel::Gemini3ProImagePreview,
                "Create a game-ready retro JRPG pixel-art sprite sheet as a transparent PNG with a true alpha background.

Subject:
A chibi armored dragon warrior with a silver horned helmet, one visible green eye, a red mane, orange armored limbs, oversized yellow claw gauntlets, and large yellow wing-like back blades.

Composition:
Front 3/4 battle view. Full character visible in every frame. Arrange multiple evenly spaced animation cells in a clean grid like a professional game sprite sheet. Keep the framing stable and the character centered consistently from frame to frame.

Action:
Include these animation poses: idle stance, walk cycle, attack pose, damage / hurt reaction, sleep / rest pose, and victory pose.

Style:
Classic late-1990s Japanese handheld monster-game pixel art. Retro JRPG battle sprite aesthetic. Crisp pixel clusters, clean dark outlines, limited saturated palette, strong silhouette, polished handheld-era sprite quality. Cute but fierce. Super-deformed proportions.

Consistency requirements:
This must be the same exact character in every frame.
Keep the same proportions, same helmet shape, same claw size, same wing shape, same body scale, same color palette, and same armor details across all poses.
Do not redesign the character between frames.
Avoid frame-to-frame drift.

Format requirements:
Transparent background only.
True PNG transparency / alpha background.
No black background.
No white background.
No checkerboard pattern.
No scenery.
No floor.
No cast shadow.
No contact shadow.
No glow halo.
No text.
No UI.
No watermark.

Quality constraints:
Make the sprite sheet look production-ready for direct use in a 2D game.
Keep edges clean and readable at small size.
Use stable spacing and alignment.
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
