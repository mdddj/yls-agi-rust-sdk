use serde_json::{Map, Value};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::Instant,
};
use yls_agi_rust_sdk::{
    ChatGptImageClient, ChatGptImageRequest, ChatGptReferenceImage, ImageMime, format_duration_ms,
};

const DEFAULT_BASE_URL: &str = "https://code.ylsagi.com/codex";
const FIXED_OUTER_MODEL: &str = "gpt-5.4";
const FIXED_IMAGE_MODEL: &str = "gpt-image-2";
const HELP_TEXT: &str = r#"Usage:
  cargo run --bin generate-image-via-responses -- \
    --prompt "A red paper-cut style dragon poster" \
    --output output/dragon \
    --api-key <token>

Options:
  --prompt <text>       Required. Prompt sent to the Responses API.
  --output <path>       Required. Output path. Extension is inferred when omitted.
  --api-key <token>     Required. API key passed explicitly on the command line.
  --reference <value>   Optional. Repeatable. Supports local file paths, http(s) URLs,
                        data URLs, and file IDs such as file-123 or file_123.
  --base-url <url>      Optional. Default: https://code.ylsagi.com/codex
  --tool-json <json>    Optional. Extra image_generation tool fields as a JSON object.
  --help                Show this help message.
"#;

type DynError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone)]
struct Options {
    prompt: String,
    output: PathBuf,
    api_key: String,
    base_url: String,
    references: Vec<String>,
    tool_overrides: Map<String, Value>,
    help: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationResult {
    output_path: String,
    absolute_output_path: String,
    inferred_extension: String,
    byte_length: usize,
    total_duration_ms: u128,
    total_duration_formatted: String,
}

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let options = parse_args(env::args().skip(1).collect())?;
    if options.help {
        println!("{HELP_TEXT}");
        return Ok(());
    }

    let client = ChatGptImageClient::with_base_url_and_auth(
        &options.api_key,
        url::Url::parse(&normalize_base_url(&options.base_url))?,
        yls_agi_rust_sdk::AuthMode::AuthorizationBearer,
    )?;

    let mut request = ChatGptImageRequest::new(FIXED_OUTER_MODEL, &options.prompt)
        .with_image_model(FIXED_IMAGE_MODEL);
    if !options.tool_overrides.is_empty() {
        request = request.with_tool_overrides(options.tool_overrides.clone());
    }
    for reference in &options.references {
        request = request.with_reference(resolve_reference(reference)?);
    }

    let started_at = Instant::now();
    let mut response = client.generate_image(request).await?;
    let inferred_extension = detect_extension_from_mime(&response.image.mime_type);
    let output_path = resolve_output_path(&options.output, inferred_extension);

    if let Some(parent) = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let saved_info = response.image.save_with_metadata(&output_path)?;
    let total_duration_ms = started_at.elapsed().as_millis();

    let result = GenerationResult {
        output_path: saved_info.output_path,
        absolute_output_path: saved_info.absolute_output_path,
        inferred_extension: inferred_extension.to_string(),
        byte_length: saved_info.byte_length,
        total_duration_ms,
        total_duration_formatted: format_duration_ms(total_duration_ms),
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn parse_args(argv: Vec<String>) -> Result<Options, DynError> {
    let mut options = Options {
        prompt: String::new(),
        output: PathBuf::new(),
        api_key: String::new(),
        base_url: DEFAULT_BASE_URL.to_string(),
        references: Vec::new(),
        tool_overrides: Map::new(),
        help: false,
    };

    let mut index = 0;
    while index < argv.len() {
        match argv[index].as_str() {
            "--prompt" => {
                options.prompt = take_option_value(&argv, index, "--prompt")?;
                index += 1;
            }
            "--output" => {
                options.output = PathBuf::from(take_option_value(&argv, index, "--output")?);
                index += 1;
            }
            "--api-key" => {
                options.api_key = take_option_value(&argv, index, "--api-key")?;
                index += 1;
            }
            "--reference" => {
                options
                    .references
                    .push(take_option_value(&argv, index, "--reference")?);
                index += 1;
            }
            "--base-url" => {
                options.base_url = take_option_value(&argv, index, "--base-url")?;
                index += 1;
            }
            "--tool-json" => {
                options.tool_overrides =
                    parse_tool_overrides(&take_option_value(&argv, index, "--tool-json")?)?;
                index += 1;
            }
            "--help" => {
                options.help = true;
            }
            unknown => {
                return Err(make_error(format!("Unknown argument: {unknown}")));
            }
        }

        index += 1;
    }

    if options.help {
        return Ok(options);
    }

    if options.prompt.trim().is_empty() {
        return Err(make_error("--prompt is required."));
    }
    if options.output.as_os_str().is_empty() {
        return Err(make_error("--output is required."));
    }
    if options.api_key.trim().is_empty() {
        return Err(make_error("--api-key is required."));
    }

    Ok(options)
}

fn take_option_value(argv: &[String], index: usize, option_name: &str) -> Result<String, DynError> {
    let Some(value) = argv.get(index + 1) else {
        return Err(make_error(format!("Missing value for {option_name}.")));
    };
    if value.starts_with("--") {
        return Err(make_error(format!("Missing value for {option_name}.")));
    }
    Ok(value.clone())
}

fn parse_tool_overrides(raw: &str) -> Result<Map<String, Value>, DynError> {
    let parsed: Value = serde_json::from_str(raw)?;
    match parsed {
        Value::Object(map) => Ok(map),
        _ => Err(make_error("--tool-json must be a JSON object.")),
    }
}

fn resolve_reference(reference: &str) -> Result<ChatGptReferenceImage, DynError> {
    let value = reference.trim();
    if value.is_empty() {
        return Err(make_error("Reference values must be non-empty strings."));
    }

    if is_data_url(value) || is_http_url(value) {
        return Ok(ChatGptReferenceImage::url(value));
    }

    if is_file_id(value) {
        return Ok(ChatGptReferenceImage::file_id(value));
    }

    let image_bytes = fs::read(value).map_err(|error| {
        make_error(format!(
            "Failed to read reference image at {value}: {error}"
        ))
    })?;
    let mime_type = detect_image_media_type(&image_bytes, Path::new(value));
    Ok(ChatGptReferenceImage::from_bytes(
        ImageMime::from(mime_type),
        image_bytes,
    ))
}

fn is_data_url(value: &str) -> bool {
    value
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

fn is_http_url(value: &str) -> bool {
    url::Url::parse(value)
        .map(|url| matches!(url.scheme(), "http" | "https"))
        .unwrap_or(false)
}

fn is_file_id(value: &str) -> bool {
    if value.contains('/') || value.contains('\\') {
        return false;
    }

    let lower = value.to_ascii_lowercase();
    let suffix = lower
        .strip_prefix("file-")
        .or_else(|| lower.strip_prefix("file_"));

    suffix.is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_alphanumeric()))
}

fn detect_image_media_type(buffer: &[u8], file_path: &Path) -> &'static str {
    if buffer.len() >= 8
        && buffer[0] == 0x89
        && buffer[1] == 0x50
        && buffer[2] == 0x4e
        && buffer[3] == 0x47
        && buffer[4] == 0x0d
        && buffer[5] == 0x0a
        && buffer[6] == 0x1a
        && buffer[7] == 0x0a
    {
        return "image/png";
    }

    if buffer.len() >= 3 && buffer[0] == 0xff && buffer[1] == 0xd8 && buffer[2] == 0xff {
        return "image/jpeg";
    }

    if buffer.len() >= 12 && &buffer[0..4] == b"RIFF" && &buffer[8..12] == b"WEBP" {
        return "image/webp";
    }

    if buffer.len() >= 6 && (&buffer[0..6] == b"GIF87a" || &buffer[0..6] == b"GIF89a") {
        return "image/gif";
    }

    match file_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("avif") => "image/avif",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn detect_extension_from_mime(mime: &str) -> &'static str {
    match mime {
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "image/webp" => ".webp",
        "image/gif" => ".gif",
        _ => ".bin",
    }
}

fn resolve_output_path(output_path: &Path, inferred_extension: &str) -> PathBuf {
    if output_path.extension().is_some() {
        return output_path.to_path_buf();
    }

    PathBuf::from(format!("{}{}", output_path.display(), inferred_extension))
}

fn normalize_base_url(base_url: &str) -> String {
    if base_url.ends_with('/') {
        base_url.to_string()
    } else {
        format!("{base_url}/")
    }
}

fn make_error(message: impl Into<String>) -> DynError {
    io::Error::other(message.into()).into()
}
