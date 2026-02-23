// src-tauri/src/sarvam.rs
// Sarvam AI API calls: Saaras v3 STT and Mayura translation

use reqwest::{multipart, Client};
use serde::Deserialize;

const STT_ENDPOINT: &str = "https://api.sarvam.ai/speech-to-text";
const TRANSLATE_ENDPOINT: &str = "https://api.sarvam.ai/translate";

// ─── STT ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SttResponse {
    pub transcript: String,
    pub language_code: String,
}

/// Send WAV bytes to Sarvam Saaras v3 STT
/// language_code: "auto" for auto-detection, or e.g. "hi-IN", "ta-IN"
pub async fn transcribe(
    api_key: &str,
    wav_bytes: Vec<u8>,
    language_code: &str,
) -> Result<SttResponse, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();

    // Build multipart form
    let file_part = multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", "saaras:v3")
        .text("with_disfluencies", "false"); // Remove um, uh, acha, haan, etc.

    // Only specify language if not auto-detect
    if language_code != "auto" {
        form = form.text("language_code", language_code.to_string());
    }

    let response = client
        .post(STT_ENDPOINT)
        .header("api-subscription-key", api_key)
        .multipart(form)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("STT API error {status}: {body}").into());
    }

    let result: SttResponse = response.json().await?;
    log::info!("Transcribed ({}) → \"{}\"", result.language_code, result.transcript);
    Ok(result)
}

// ─── Translation ──────────────────────────────────────────────────────────────

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

/// Translate text using Sarvam Mayura v1
pub async fn translate(
    api_key: &str,
    text: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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

/// Lightweight check: just hits the translate endpoint with a short string
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
