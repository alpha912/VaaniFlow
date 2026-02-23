// src-tauri/src/sarvam.rs
// Sarvam AI API calls: STT, STT-Translate, and Text Translation

use reqwest::{multipart, Client};
use serde::Deserialize;

const STT_ENDPOINT: &str = "https://api.sarvam.ai/speech-to-text";
const STT_TRANSLATE_ENDPOINT: &str = "https://api.sarvam.ai/speech-to-text-translate";
const TRANSLATE_ENDPOINT: &str = "https://api.sarvam.ai/translate";

/// Reusable HTTP/1.1 client — some multipart APIs don't work with HTTP/2
fn http1_client() -> Result<Client, reqwest::Error> {
    Client::builder().http1_only().build()
}

// ─── STT ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SttResponse {
    pub transcript: String,
    pub language_code: String,
}

/// Send WAV bytes to Sarvam Saarika STT (speech-to-text only)
/// language_code: "auto" for auto-detection, or e.g. "hi-IN", "ta-IN"
pub async fn transcribe(
    api_key: &str,
    wav_bytes: Vec<u8>,
    language_code: &str,
) -> Result<SttResponse, Box<dyn std::error::Error + Send + Sync>> {
    log::info!("Calling STT API: {} bytes, lang={}", wav_bytes.len(), language_code);

    let client = http1_client()?;

    let file_part = multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", "saarika:v2.5");

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

// ─── STT + Translate (single API call, supports English output) ──────────────

#[derive(Debug, Deserialize)]
pub struct SttTranslateResponse {
    pub transcript: String,
    pub language_code: String,
}

/// Send WAV bytes to Sarvam Saaras STT-Translate (speech → English text)
/// Uses the /speech-to-text-translate endpoint with model=saaras:v3
pub async fn transcribe_translate(
    api_key: &str,
    wav_bytes: Vec<u8>,
    language_code: &str,
) -> Result<SttTranslateResponse, Box<dyn std::error::Error + Send + Sync>> {
    log::info!("Calling STT-Translate API: {} bytes, lang={}", wav_bytes.len(), language_code);

    let client = http1_client()?;

    let file_part = multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("model", "saaras:v3")
        .text("mode", "classic");

    if language_code != "auto" {
        form = form.text("language_code", language_code.to_string());
    }

    let response = client
        .post(STT_TRANSLATE_ENDPOINT)
        .header("api-subscription-key", api_key)
        .multipart(form)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("STT-Translate API error {status}: {body}").into());
    }

    let result: SttTranslateResponse = response.json().await?;
    log::info!("STT-Translated ({}) → \"{}\"", result.language_code, result.transcript);
    Ok(result)
}

// ─── Text Translation ────────────────────────────────────────────────────────

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

/// Translate text using Sarvam Mayura v1 (text → text)
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
