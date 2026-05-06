use futures::StreamExt;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, header, method, path},
};
use yls_agi_rust_sdk::{
    AuthMode, ChatMessage, ChatRequest, ClaudeClient, Client, GeminiClient, GeminiImageRequest,
    GeminiModel, ImageMimeType, OpenAiClient, Provider,
};

#[tokio::test]
async fn openai_json_request_shape_is_correct() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/openai/v1/chat/completions"))
        .and(header("authorization", "Bearer test-key"))
        .and(body_json(json!({
            "model": "gpt-4.1",
            "messages": [
                {"role": "system", "content": [{"type": "text", "text": "Be terse."}]},
                {"role": "user", "content": [{"type": "text", "text": "hi"}]}
            ],
            "stream": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "data: {\"choices\":[{\"delta\":{\"content\":\"he\"},\"finish_reason\":null}]}\n\n\
             data: {\"choices\":[{\"delta\":{\"content\":\"llo\"},\"finish_reason\":\"stop\"}]}\n\n\
             data: [DONE]\n\n",
        ))
        .mount(&server)
        .await;

    let client = OpenAiClient::with_base_url_and_auth(
        "test-key",
        format!("{}/openai/v1/", server.uri()).parse().unwrap(),
        AuthMode::AuthorizationBearer,
    )
    .unwrap();

    let request = ChatRequest::new(
        "gpt-4.1",
        vec![ChatMessage::system("Be terse."), ChatMessage::user("hi")],
    )
    .with_stream(true);

    let mut stream = client.chat_stream(request).await.unwrap();
    let chunks = stream.by_ref().collect::<Vec<_>>().await;
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].as_ref().unwrap().delta, "he");
    assert_eq!(chunks[1].as_ref().unwrap().delta, "llo");
    assert!(chunks[1].as_ref().unwrap().done);
}

#[tokio::test]
async fn claude_json_request_shape_is_correct() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/claude/v1/messages"))
        .and(header("authorization", "test-key"))
        .and(body_json(json!({
            "model": "claude-sonnet-4-5-20250929",
            "stream": false,
            "max_tokens": 8192u32,
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "hi"}]}
            ],
            "system": "Be terse."
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "claude-sonnet-4-5-20250929",
            "content": [{"type": "text", "text": "hello"}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 2, "output_tokens": 1}
        })))
        .mount(&server)
        .await;

    let client = ClaudeClient::with_base_url_and_auth(
        "test-key",
        format!("{}/claude/v1/", server.uri()).parse().unwrap(),
        AuthMode::AuthorizationKey,
    )
    .unwrap();

    let request = ChatRequest::new(
        "claude-sonnet-4-5-20250929",
        vec![ChatMessage::system("Be terse."), ChatMessage::user("hi")],
    );

    let response = client.chat(request).await.unwrap();
    assert_eq!(response.message.text_content(), "hello");
    assert_eq!(response.usage.unwrap().total_tokens, Some(3));
}

#[tokio::test]
async fn openai_multimodal_request_shape_is_correct() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/openai/v1/chat/completions"))
        .and(header("authorization", "Bearer test-key"))
        .and(body_json(json!({
            "model": "gpt-4.1",
            "messages": [
                {"role": "user", "content": [
                    {"type": "text", "text": "describe this image"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,aGVsbG8="}}
                ]}
            ],
            "stream": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\n\
             data: [DONE]\n\n",
        ))
        .mount(&server)
        .await;

    let client = OpenAiClient::with_base_url_and_auth(
        "test-key",
        format!("{}/openai/v1/", server.uri()).parse().unwrap(),
        AuthMode::AuthorizationBearer,
    )
    .unwrap();

    let request = ChatRequest::new(
        "gpt-4.1",
        vec![
            ChatMessage::user("describe this image")
                .with_image_base64(ImageMimeType::Png, "aGVsbG8="),
        ],
    )
    .with_stream(true);

    let chunks = client
        .chat_stream(request)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(chunks[0].as_ref().unwrap().delta, "ok");
}

#[tokio::test]
async fn claude_multimodal_request_shape_is_correct() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/claude/v1/messages"))
        .and(header("authorization", "test-key"))
        .and(body_json(json!({
            "model": "claude-sonnet-4-5-20250929",
            "stream": false,
            "max_tokens": 8192u32,
            "messages": [
                {"role": "user", "content": [
                    {"type": "text", "text": "describe this image"},
                    {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "aGVsbG8="}}
                ]}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "claude-sonnet-4-5-20250929",
            "content": [{"type": "text", "text": "ok"}],
            "stop_reason": "end_turn"
        })))
        .mount(&server)
        .await;

    let client = ClaudeClient::with_base_url_and_auth(
        "test-key",
        format!("{}/claude/v1/", server.uri()).parse().unwrap(),
        AuthMode::AuthorizationKey,
    )
    .unwrap();

    let request = ChatRequest::new(
        "claude-sonnet-4-5-20250929",
        vec![
            ChatMessage::user("describe this image")
                .with_image_base64(ImageMimeType::Png, "aGVsbG8="),
        ],
    );
    let response = client.chat(request).await.unwrap();
    assert_eq!(response.message.text_content(), "ok");
}

#[tokio::test]
async fn claude_sse_is_parsed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/claude/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"he\"}}\n\n\
             data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"llo\"}}\n\n\
             data: {\"type\":\"message_stop\"}\n\n",
        ))
        .mount(&server)
        .await;

    let client = ClaudeClient::with_base_url_and_auth(
        "test-key",
        format!("{}/claude/v1/", server.uri()).parse().unwrap(),
        AuthMode::AuthorizationKey,
    )
    .unwrap();

    let request = ChatRequest::new("claude-sonnet-4-5-20250929", vec![ChatMessage::user("hi")])
        .with_stream(true);
    let chunks = client
        .chat_stream(request)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].as_ref().unwrap().delta, "he");
    assert_eq!(chunks[1].as_ref().unwrap().delta, "llo");
    assert!(chunks[2].as_ref().unwrap().done);
}

#[tokio::test]
async fn gemini_json_request_shape_is_correct() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/gemini/v1beta/models/gemini-2.5-flash:generateContent",
        ))
        .and(header("x-goog-api-key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "hello"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 2,
                "candidatesTokenCount": 1,
                "totalTokenCount": 3
            }
        })))
        .mount(&server)
        .await;

    let client = GeminiClient::with_base_url_and_auth(
        "test-key",
        format!("{}/gemini/v1beta/", server.uri()).parse().unwrap(),
        AuthMode::XGoogApiKey,
    )
    .unwrap();

    let request = ChatRequest::new("gemini-2.5-flash", vec![ChatMessage::user("hi")]);
    let response = client.chat(request).await.unwrap();
    assert_eq!(response.message.text_content(), "hello");
    assert_eq!(response.usage.unwrap().total_tokens, Some(3));
}

#[tokio::test]
async fn gemini_multimodal_request_shape_is_correct() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/gemini/v1beta/models/gemini-3-pro-preview:generateContent",
        ))
        .and(header("x-goog-api-key", "test-key"))
        .and(body_json(json!({
            "contents": [{
                "role": "user",
                "parts": [
                    {"text": "describe this image"},
                    {"inlineData": {"mimeType": "image/png", "data": "aGVsbG8="}}
                ]
            }]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "hello image"}]
                },
                "finishReason": "STOP"
            }]
        })))
        .mount(&server)
        .await;

    let client = GeminiClient::with_base_url_and_auth(
        "test-key",
        format!("{}/gemini/v1beta/", server.uri()).parse().unwrap(),
        AuthMode::XGoogApiKey,
    )
    .unwrap();

    let request = ChatRequest::new(
        GeminiModel::Gemini3ProPreview,
        vec![
            ChatMessage::user("describe this image")
                .with_image_base64(ImageMimeType::Png, "aGVsbG8="),
        ],
    );
    let response = client.chat(request).await.unwrap();
    assert_eq!(response.message.text_content(), "hello image");
}

#[tokio::test]
async fn gemini_image_generation_is_parsed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/gemini/v1beta/models/gemini-2.5-flash-image:generateContent",
        ))
        .and(header("x-goog-api-key", "test-key"))
        .and(body_json(json!({
            "contents": [{
                "parts": [{"text": "draw a cat"}]
            }],
            "generationConfig": {
                "responseModalities": ["TEXT", "IMAGE"]
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "done"},
                        {"inlineData": {"mimeType": "image/png", "data": "aGVsbG8="}}
                    ]
                }
            }]
        })))
        .mount(&server)
        .await;

    let client = GeminiClient::with_base_url_and_auth(
        "test-key",
        format!("{}/gemini/v1beta/", server.uri()).parse().unwrap(),
        AuthMode::XGoogApiKey,
    )
    .unwrap();

    let response = client
        .generate_image(GeminiImageRequest::new(
            GeminiModel::Gemini25FlashImage,
            "draw a cat",
        ))
        .await
        .unwrap();

    assert_eq!(response.text, vec!["done".to_string()]);
    assert_eq!(response.images.len(), 1);
    assert_eq!(response.images[0].mime_type, "image/png");
    assert_eq!(response.images[0].bytes, b"hello".to_vec());
}

#[tokio::test]
async fn gemini_image_generation_with_reference_images_is_serialized() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/gemini/v1beta/models/gemini-3-pro-image-preview:generateContent",
        ))
        .and(header("x-goog-api-key", "test-key"))
        .and(body_json(json!({
            "contents": [{
                "parts": [
                    {"text": "turn this into a pixel-art game card"},
                    {"inlineData": {"mimeType": "image/png", "data": "aGVsbG8="}}
                ]
            }],
            "generationConfig": {
                "responseModalities": ["TEXT", "IMAGE"]
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "done"},
                        {"inlineData": {"mimeType": "image/png", "data": "aGVsbG8="}}
                    ]
                }
            }]
        })))
        .mount(&server)
        .await;

    let client = GeminiClient::with_base_url_and_auth(
        "test-key",
        format!("{}/gemini/v1beta/", server.uri()).parse().unwrap(),
        AuthMode::XGoogApiKey,
    )
    .unwrap();

    let response = client
        .generate_image(
            GeminiImageRequest::new(
                GeminiModel::Gemini3ProImagePreview,
                "turn this into a pixel-art game card",
            )
            .with_reference_image_base64(ImageMimeType::Png, "aGVsbG8="),
        )
        .await
        .unwrap();

    assert_eq!(response.text, vec!["done".to_string()]);
    assert_eq!(response.images.len(), 1);
}

#[tokio::test]
async fn unified_client_routes_to_provider() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/claude/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "claude-sonnet-4-5-20250929",
            "content": [{"type": "text", "text": "routed"}],
            "stop_reason": "end_turn"
        })))
        .mount(&server)
        .await;

    let client = Client::builder("test-key")
        .with_claude_base_url(format!("{}/claude/v1/", server.uri()))
        .build()
        .unwrap();

    let response = client
        .chat(
            Provider::Claude,
            ChatRequest::new("claude-sonnet-4-5-20250929", vec![ChatMessage::user("hi")]),
        )
        .await
        .unwrap();

    assert_eq!(response.message.text_content(), "routed");
}
