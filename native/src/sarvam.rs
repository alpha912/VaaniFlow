//! Sarvam REST API adapter. Never log API keys, request bodies, transcripts, or
//! raw server responses: they may contain sensitive dictation.

use crate::config::Config;
use reqwest::{multipart, Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::{sync::LazyLock, time::Duration};

const STT_ENDPOINT: &str = "https://api.sarvam.ai/speech-to-text";
const TRANSLATE_ENDPOINT: &str = "https://api.sarvam.ai/translate";
static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(45))
        .user_agent(concat!("VaaniFlow/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("reqwest client configuration is valid")
});

type ApiResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, Deserialize)]
pub struct SttResponse {
    pub transcript: String,
    #[serde(default)]
    pub language_code: Option<String>,
}

#[derive(Debug, Serialize)]
struct TranslateRequest<'a> {
    input: &'a str,
    source_language_code: &'a str,
    target_language_code: &'a str,
    model: &'static str,
    mode: &'a str,
}

#[derive(Debug, Deserialize)]
struct TranslateResponse {
    translated_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Transcribe { mode: String },
    TranscribeThenTranslate { target: String },
}

pub fn route_for(config: &Config) -> Route {
    match config.target_language.as_str() {
        "same" => Route::Transcribe {
            mode: config.stt_mode.clone(),
        },
        "en-IN" => Route::Transcribe {
            mode: "translate".into(),
        },
        target => Route::TranscribeThenTranslate {
            target: target.to_owned(),
        },
    }
}

pub async fn transcribe_and_translate(
    api_key: &str,
    wav_bytes: Vec<u8>,
    config: &Config,
) -> ApiResult<String> {
    if api_key.trim().is_empty() {
        return Err("Add your Sarvam API key in Settings first.".into());
    }
    let route = route_for(config);
    let (mode, translate_target) = match route {
        Route::Transcribe { mode } => (mode, None),
        Route::TranscribeThenTranslate { target } => ("transcribe".to_owned(), Some(target)),
    };

    let stt = speech_to_text(
        api_key,
        wav_bytes,
        &config.source_language,
        &config.stt_model,
        &mode,
        &config.keyterms,
    )
    .await?;
    let Some(target) = translate_target else {
        return Ok(stt.transcript);
    };

    let detected = stt.language_code.as_deref().unwrap_or("auto");
    if target == detected {
        return Ok(stt.transcript);
    }
    translate(
        api_key,
        &stt.transcript,
        detected,
        &target,
        &config.translation_mode,
    )
    .await
}

pub async fn speech_to_text(
    api_key: &str,
    wav_bytes: Vec<u8>,
    language_code: &str,
    model: &str,
    mode: &str,
    keyterms: &str,
) -> ApiResult<SttResponse> {
    if !matches!(model, "saaras:v3" | "saaras:v4") {
        return Err("Choose a supported Sarvam speech model.".into());
    }
    if !matches!(
        mode,
        "transcribe" | "translate" | "verbatim" | "translit" | "codemix"
    ) {
        return Err("Choose a supported speech output mode.".into());
    }

    let audio = multipart::Part::bytes(wav_bytes)
        .file_name("vaaniflow-recording.wav")
        .mime_str("audio/wav")?;
    let mut form = multipart::Form::new()
        .part("file", audio)
        .text("model", model.to_owned())
        .text("mode", mode.to_owned());
    if language_code != "auto" {
        form = form.text("language_code", language_code.to_owned());
    }
    if model == "saaras:v4" && !keyterms.trim().is_empty() {
        let terms = keyterms
            .split(',')
            .map(str::trim)
            .filter(|term| !term.is_empty())
            .take(50)
            .collect::<Vec<_>>();
        if !terms.is_empty() {
            form = form.text("keyterms", serde_json::to_string(&terms)?);
        }
    }

    let response = HTTP_CLIENT
        .post(STT_ENDPOINT)
        .header("api-subscription-key", api_key)
        .multipart(form)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(api_error("Speech-to-text", response.status()));
    }
    let result: SttResponse = response.json().await?;
    if result.transcript.trim().is_empty() {
        return Err("Sarvam returned an empty transcript. Try recording again.".into());
    }
    Ok(result)
}

pub async fn translate(
    api_key: &str,
    text: &str,
    source_lang: &str,
    target_lang: &str,
    translation_mode: &str,
) -> ApiResult<String> {
    if text.chars().count() > 1000 {
        return Err("This transcript is too long for one Mayura v1 translation request. Choose a shorter recording.".into());
    }
    let mode = match translation_mode {
        "formal" | "classic-colloquial" | "modern-colloquial" | "code-mixed" => translation_mode,
        _ => "formal",
    };
    let body = TranslateRequest {
        input: text,
        source_language_code: if source_lang == "auto" {
            "auto"
        } else {
            source_lang
        },
        target_language_code: target_lang,
        model: "mayura:v1",
        mode,
    };
    let response = HTTP_CLIENT
        .post(TRANSLATE_ENDPOINT)
        .header("api-subscription-key", api_key)
        .json(&body)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(api_error("Translation", response.status()));
    }
    let result: TranslateResponse = response.json().await?;
    Ok(result.translated_text)
}

pub async fn test_key(api_key: &str) -> ApiResult<()> {
    let body = TranslateRequest {
        input: "test",
        source_language_code: "en-IN",
        target_language_code: "hi-IN",
        model: "mayura:v1",
        mode: "formal",
    };
    let response = HTTP_CLIENT
        .post(TRANSLATE_ENDPOINT)
        .header("api-subscription-key", api_key)
        .json(&body)
        .send()
        .await?;
    if response.status() == StatusCode::UNAUTHORIZED || response.status() == StatusCode::FORBIDDEN {
        return Err("Sarvam rejected this API key. Check it and try again.".into());
    }
    if !response.status().is_success() {
        return Err(api_error("API key test", response.status()));
    }
    Ok(())
}

fn api_error(operation: &str, status: StatusCode) -> Box<dyn std::error::Error + Send + Sync> {
    let message = match status.as_u16() {
        400 | 422 => "The request was rejected. Check language, model, and audio format settings.",
        401 | 403 => "Sarvam rejected the API key. Check your key and account access.",
        429 => "Sarvam rate limit reached. Wait a moment and try again.",
        500..=599 => "Sarvam is temporarily unavailable. Try again shortly.",
        _ => "The request failed. Check your connection and try again.",
    };
    format!("{operation} failed (HTTP {status}): {message}").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_target_language_correctly() {
        let mut config = Config::default();
        assert_eq!(
            route_for(&config),
            Route::Transcribe {
                mode: "transcribe".into()
            }
        );
        config.target_language = "en-IN".into();
        assert_eq!(
            route_for(&config),
            Route::Transcribe {
                mode: "translate".into()
            }
        );
        config.target_language = "ta-IN".into();
        assert_eq!(
            route_for(&config),
            Route::TranscribeThenTranslate {
                target: "ta-IN".into()
            }
        );
    }

    #[test]
    fn server_errors_never_include_response_body() {
        let message = api_error("Translation", StatusCode::INTERNAL_SERVER_ERROR).to_string();
        assert!(message.contains("HTTP 500"));
        assert!(!message.contains("transcript"));
    }
}
