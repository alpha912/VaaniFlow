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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionResult {
    /// The final text, or the original transcript if optional translation fails.
    pub text: String,
    /// Present when speech recognition succeeded but translation did not.
    pub translation_warning: Option<String>,
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
    wav_segments: Vec<Vec<u8>>,
    config: &Config,
) -> ApiResult<TranscriptionResult> {
    transcribe_and_translate_at(
        api_key,
        wav_segments,
        config,
        STT_ENDPOINT,
        TRANSLATE_ENDPOINT,
    )
    .await
}

async fn transcribe_and_translate_at(
    api_key: &str,
    wav_segments: Vec<Vec<u8>>,
    config: &Config,
    stt_endpoint: &str,
    translate_endpoint: &str,
) -> ApiResult<TranscriptionResult> {
    if api_key.trim().is_empty() {
        return Err("Add your Sarvam API key in Settings first.".into());
    }
    if wav_segments.is_empty() {
        return Err("No audio was captured. Record a short phrase and try again.".into());
    }
    let route = route_for(config);
    let (mode, translate_target) = match route {
        Route::Transcribe { mode } => (mode, None),
        Route::TranscribeThenTranslate { target } => ("transcribe".to_owned(), Some(target)),
    };

    let mut transcriptions = Vec::with_capacity(wav_segments.len());
    for wav in wav_segments {
        transcriptions.push(
            speech_to_text_with_endpoint(
                api_key,
                wav,
                &config.source_language,
                &config.stt_model,
                &mode,
                &config.keyterms,
                stt_endpoint,
            )
            .await?,
        );
    }

    let transcript = transcriptions
        .iter()
        .map(|segment| segment.transcript.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let Some(target) = translate_target else {
        return Ok(TranscriptionResult {
            text: transcript,
            translation_warning: None,
        });
    };

    let mut translated_segments = Vec::with_capacity(transcriptions.len());
    for segment in &transcriptions {
        let detected = segment.language_code.as_deref().unwrap_or("auto");
        if target == detected {
            translated_segments.push(segment.transcript.clone());
            continue;
        }
        match translate_with_endpoint(
            api_key,
            &segment.transcript,
            detected,
            &target,
            &config.translation_mode,
            translate_endpoint,
        )
        .await
        {
            Ok(text) => translated_segments.push(text),
            Err(error) => return Ok(translation_fallback(transcript, error.to_string())),
        }
    }
    Ok(TranscriptionResult {
        text: translated_segments.join(" "),
        translation_warning: None,
    })
}

fn translation_fallback(transcript: String, error: String) -> TranscriptionResult {
    TranscriptionResult {
        text: transcript,
        translation_warning: Some(format!(
            "Translation failed; the original transcript is shown instead. {error}"
        )),
    }
}

async fn speech_to_text_with_endpoint(
    api_key: &str,
    wav_bytes: Vec<u8>,
    language_code: &str,
    model: &str,
    mode: &str,
    keyterms: &str,
    endpoint: &str,
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
        .post(endpoint)
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

async fn translate_with_endpoint(
    api_key: &str,
    text: &str,
    source_lang: &str,
    target_lang: &str,
    translation_mode: &str,
    endpoint: &str,
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
        .post(endpoint)
        .header("api-subscription-key", api_key)
        .json(&body)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(api_error("Translation", response.status()));
    }
    let result: TranslateResponse = response.json().await?;
    if result.translated_text.trim().is_empty() {
        return Err(
            "Sarvam returned an empty translation. The original transcript is preserved.".into(),
        );
    }
    Ok(result.translated_text)
}

/// Verify access to Mayura translation. This does not verify Saaras STT access.
pub async fn test_translation_key(api_key: &str) -> ApiResult<()> {
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
        return Err(api_error("Translation access test", response.status()));
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
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread::JoinHandle,
        time::Duration,
    };

    fn mock_server(responses: Vec<(u16, &'static str)>) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            responses
                .into_iter()
                .map(|(status, body)| {
                    let (mut stream, _) = listener.accept().unwrap();
                    let path = read_request(&mut stream);
                    let reason = if status == 200 { "OK" } else { "Service Unavailable" };
                    write!(
                        stream,
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                    path
                })
                .collect()
        });
        (format!("http://{address}"), server)
    }

    fn read_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        let header_end = loop {
            let count = stream.read(&mut buffer).unwrap();
            assert_ne!(count, 0, "request ended before its headers");
            request.extend_from_slice(&buffer[..count]);
            if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let headers = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        while request.len() < header_end + content_length {
            let count = stream.read(&mut buffer).unwrap();
            assert_ne!(count, 0, "request ended before its body");
            request.extend_from_slice(&buffer[..count]);
        }
        String::from_utf8_lossy(&request[..header_end])
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .to_owned()
    }

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

    #[test]
    fn translation_failure_returns_the_full_original_transcript() {
        let transcript = "first segment second segment".to_owned();
        let result = translation_fallback(transcript.clone(), "service unavailable".into());
        assert_eq!(result.text, transcript);
        assert!(result
            .translation_warning
            .as_deref()
            .is_some_and(|warning| warning.contains("service unavailable")));
    }

    #[test]
    fn api_translation_failure_preserves_transcript_end_to_end() {
        let (base, server) = mock_server(vec![
            (
                200,
                r#"{"transcript":"namaste duniya","language_code":"hi-IN"}"#,
            ),
            (503, r#"{"message":"temporary failure"}"#),
        ]);
        let config = Config {
            source_language: "hi-IN".into(),
            target_language: "ta-IN".into(),
            ..Config::default()
        };

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime
            .block_on(transcribe_and_translate_at(
                "test-key",
                vec![b"test wav".to_vec()],
                &config,
                &format!("{base}/speech-to-text"),
                &format!("{base}/translate"),
            ))
            .unwrap();

        assert_eq!(result.text, "namaste duniya");
        assert!(result
            .translation_warning
            .as_deref()
            .is_some_and(|warning| warning.contains("HTTP 503")));
        assert_eq!(
            server.join().unwrap(),
            vec!["/speech-to-text", "/translate"]
        );
    }
}
