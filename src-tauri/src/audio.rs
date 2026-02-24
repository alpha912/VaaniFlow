// src-tauri/src/audio.rs
// Microphone capture using cpal, WAV encoding using hound

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Cursor;
use std::sync::{Arc, Mutex};

pub struct Recorder {
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Option<cpal::Stream>,
    sample_rate: u32,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            sample_rate: 16000,
        }
    }

    /// Begin capturing from the specified or default input device
    pub fn start(&mut self, microphone: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        {
            let mut s = self.samples.lock().unwrap();
            s.clear();
        }

        let host = cpal::default_host();

        // Log all available input devices
        let available: Vec<_> = host.input_devices()?.collect();
        for d in &available {
            let name = d.name().unwrap_or_else(|_| "unknown".into());
            log::info!("Available input device: {}", name);
        }

        // Select device: by name match or system default
        let device = if microphone == "default" {
            host.default_input_device()
                .ok_or("No input device found. Check microphone permissions.")?
        } else {
            available
                .into_iter()
                .find(|d| {
                    d.name()
                        .map(|n| n.contains(microphone))
                        .unwrap_or(false)
                })
                .ok_or_else(|| format!("Microphone '{}' not found. Check Settings.", microphone))?
        };

        let device_name = device.name().unwrap_or_else(|_| "unknown".into());
        log::info!("Selected input device: {}", device_name);

        // Use device's default config to avoid format mismatch issues
        let default_config = device.default_input_config()?;
        log::info!("Using config: {}Hz, {} ch, {:?}",
            default_config.sample_rate().0, default_config.channels(), default_config.sample_format());
        let config: cpal::StreamConfig = default_config.into();

        self.sample_rate = config.sample_rate.0;
        let channels = config.channels as usize;
        let samples_arc = self.samples.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                let mut buf = samples_arc.lock().unwrap();
                // Downmix to mono by averaging channels
                for chunk in data.chunks(channels) {
                    let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                    buf.push(mono);
                }
            },
            |err| log::error!("Audio stream error: {err}"),
            None,
        )?;

        stream.play()?;
        self.stream = Some(stream);
        log::info!("Recording started at {}Hz, {} channels", self.sample_rate, channels);
        Ok(())
    }

    /// Stop capture and return (samples, sample_rate)
    pub fn stop(&mut self) -> Result<(Vec<f32>, u32), Box<dyn std::error::Error + Send + Sync>> {
        drop(self.stream.take());

        let samples = std::mem::take(&mut *self.samples.lock().unwrap());
        let rate = self.sample_rate;

        // Log audio amplitude stats for debugging
        if !samples.is_empty() {
            let max_amp = samples.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
            let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
            log::info!(
                "Recording stopped. {} samples ({:.1}s) at {}Hz — max={:.4} rms={:.4}",
                samples.len(), samples.len() as f32 / rate as f32, rate, max_amp, rms
            );
        } else {
            log::info!("Recording stopped. 0 samples");
        }

        Ok((samples, rate))
    }
}

/// List all available input device names
pub fn list_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                names.push(name);
            }
        }
    }
    names
}

/// Encode f32 samples to WAV bytes in memory.
/// Always outputs 16kHz 16-bit PCM mono WAV (resamples if needed).
pub fn encode_wav(
    samples: Vec<f32>,
    sample_rate: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // Resample to 16kHz if necessary
    let (final_samples, final_rate) = if sample_rate != 16000 {
        let resampled = resample(&samples, sample_rate, 16000);
        log::info!(
            "Resampled {}Hz → 16000Hz ({} → {} samples)",
            sample_rate, samples.len(), resampled.len()
        );
        (resampled, 16000)
    } else {
        (samples, sample_rate)
    };

    let mut cursor = Cursor::new(Vec::new());

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: final_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::new(&mut cursor, spec)?;

    for &sample in &final_samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(pcm)?;
    }

    writer.finalize()?;
    Ok(cursor.into_inner())
}

/// Simple linear interpolation resampler
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;

        let sample = if src_idx + 1 < samples.len() {
            samples[src_idx] as f64 * (1.0 - frac) + samples[src_idx + 1] as f64 * frac
        } else {
            samples[src_idx] as f64
        };

        output.push(sample as f32);
    }

    output
}

// Ensure Recorder is Send (stream is Send on all platforms we target)
unsafe impl Send for Recorder {}
