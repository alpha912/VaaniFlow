// src-tauri/src/audio.rs
// Microphone capture using cpal, WAV encoding using hound

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Cursor;
use std::sync::{Arc, Mutex};

pub struct Recorder {
    // Samples collected during recording (f32, mono, 16kHz target)
    samples: Arc<Mutex<Vec<f32>>>,
    // Active stream (dropped to stop recording)
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

    /// Begin capturing from the default input device
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Clear previous recording
        {
            let mut s = self.samples.lock().unwrap();
            s.clear();
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device found. Check microphone permissions.")?;

        let mut supported_configs = device.supported_input_configs()?;
        let supported_config = supported_configs
            .next()
            .ok_or("No supported input config")?;

        // Prefer 16kHz mono if supported; otherwise take device default
        let config = if supported_config.min_sample_rate().0 <= 16000
            && supported_config.max_sample_rate().0 >= 16000
        {
            cpal::StreamConfig {
                channels: 1,
                sample_rate: cpal::SampleRate(16000),
                buffer_size: cpal::BufferSize::Default,
            }
        } else {
            supported_config.with_max_sample_rate().into()
        };

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
        log::info!("Recording started at {}Hz", self.sample_rate);
        Ok(())
    }

    /// Stop capture and return collected samples
    pub fn stop(&mut self) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
        // Drop the stream to stop capture
        drop(self.stream.take());

        let samples = self.samples.lock().unwrap().clone();
        log::info!("Recording stopped. {} samples ({:.1}s)", samples.len(),
            samples.len() as f32 / self.sample_rate as f32);
        Ok(samples)
    }
}

/// Encode f32 samples to WAV bytes in memory
/// Sarvam expects 16-bit PCM WAV at 16kHz mono
pub fn encode_wav(
    samples: Vec<f32>,
    sample_rate: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut cursor = Cursor::new(Vec::new());

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::new(&mut cursor, spec)?;

    for &sample in &samples {
        // Clamp and scale f32 [-1.0, 1.0] → i16
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(pcm)?;
    }

    writer.finalize()?;
    Ok(cursor.into_inner())
}

// Ensure Recorder is Send (stream is Send on all platforms we target)
unsafe impl Send for Recorder {}
