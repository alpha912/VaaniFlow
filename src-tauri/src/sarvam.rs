// src-tauri/src/sarvam.rs
// Sarvam AI API calls using Saaras v3 model exclusively

use reqwest::Client;
use serde::Deserialize;

const STT_ENDPOINT: &str = "https://api.sarvam.ai/speech-to-text";
const TRANSLATE_ENDPOINT: &str = "https://api.sarvam.ai/translate";

/// Saaras v3 output modes (via `mode` form field)
pub enum SttMode {
    /// Standard transcription in original language
    Transcribe,
    /// Translate speech directly to English
    Translate,
}

impl SttMode {
    fn as_str(&self) -> &'static str {
        match self {
            SttMode::Transcribe => "transcribe",
            SttMode::Translate => "translate",
        }
    }
}

/// POST a file via curl -F (reqwest multipart doesn't work with Sarvam's parser).
async fn curl_multipart_post(
    url: &str,
    api_key: &str,
    wav_bytes: &[u8],
    fields: &[(&str, &str)],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let tmp_path = std::env::temp_dir().join("vaani_audio.wav");
    std::fs::write(&tmp_path, wav_bytes)?;

    let mut args = vec![
        "-s".to_string(),
        "-X".to_string(), "POST".to_string(),
        url.to_string(),
        "-H".to_string(), format!("api-subscription-key: {api_key}"),
        "-F".to_string(), format!("file=@{}", tmp_path.display()),
    ];

    for (name, value) in fields {
        args.push("-F".to_string());
        args.push(format!("{name}={value}"));
    }

    log::info!("curl POST {} with {} form fields", url, fields.len() + 1);

    let output = tokio::process::Command::new("curl")
        .args(&args)
        .output()
        .await?;

    let _ = std::fs::remove_file(&tmp_path);

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() {
        log::warn!("curl stderr: {stderr}");
    }

    if !output.status.success() {
        return Err(format!("curl failed (exit {}): {stderr}", output.status).into());
    }

    log::info!("curl response: {stdout}");

    if stdout.contains("\"error\"") {
        return Err(format!("API error: {stdout}").into());
    }

    Ok(stdout)
}

// ─── Speech-to-Text (Saaras v3) ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SttResponse {
    pub transcript: String,
    #[serde(default)]
    pub language_code: Option<String>,
}

/// Send WAV bytes to Sarvam Saaras v3 STT.
/// mode: Transcribe (same language) or Translate (→ English)
pub async fn speech_to_text(
    api_key: &str,
    wav_bytes: Vec<u8>,
    language_code: &str,
    mode: SttMode,
) -> Result<SttResponse, Box<dyn std::error::Error + Send + Sync>> {
    let mode_str = mode.as_str();
    log::info!("Calling Saaras v3 STT: {} bytes, lang={}, mode={}", wav_bytes.len(), language_code, mode_str);

    let mut fields: Vec<(&str, &str)> = vec![
        ("model", "saaras:v3"),
        ("mode", mode_str),
    ];

    if language_code != "auto" {
        fields.push(("language_code", language_code));
    }

    let json = curl_multipart_post(STT_ENDPOINT, api_key, &wav_bytes, &fields).await?;
    let result: SttResponse = serde_json::from_str(&json)?;
    log::info!("Saaras v3 ({:?}, {}) → \"{}\"", result.language_code, mode_str, result.transcript);
    Ok(result)
}

// ─── Text Translation (Mayura v1) ───────────────────────────────────────────

#[derive(serde::Serialize)]
struct TranslateRequest<'a> {
    input: &'a str,
    source_language_code: &'a str,
    target_language_code: &'a str,
    model: &'static str,
    mode: &'static str,
}

#[derive(Deserialize)]
struct TranslateResponse {
    translated_text: String,
}

/// Translate text using Sarvam Mayura v1 (text → text, for non-English targets)
pub async fn translate(
    api_key: &str,
    text: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log::info!("Translate: {} → {} (\"{}\")", source_lang, target_lang, text);
    let client = Client::new();

    let body = TranslateRequest {
        input: text,
        source_language_code: source_lang,
        target_language_code: target_lang,
        model: "mayura:v1",
        mode: "formal",
    };

    let response = client
        .post(TRANSLATE_ENDPOINT)
        .header("api-subscription-key", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Translate API error {status}: {body}").into());
    }

    let result: TranslateResponse = response.json().await?;
    log::info!("Translated → \"{}\"", result.translated_text);
    Ok(result.translated_text)
}

// ─── API key validation ───────────────────────────────────────────────────────

pub async fn test_key(api_key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();

    let body = serde_json::json!({
        "input": "नमस्ते",
        "source_language_code": "hi-IN",
        "target_language_code": "en-IN",
        "model": "mayura:v1",
        "mode": "formal"
    });

    let response = client
        .post(TRANSLATE_ENDPOINT)
        .header("api-subscription-key", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if response.status() == 401 || response.status() == 403 {
        return Err("Invalid API key".into());
    }
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API error {status}: {body}").into());
    }

    Ok(())
}
