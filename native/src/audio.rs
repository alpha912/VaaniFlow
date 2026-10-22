//! Microphone capture, bounded to stay below Sarvam's REST duration limit.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

/// Keep a few seconds below the REST API's 30-second guidance.
pub const MAX_RECORDING_SECONDS: usize = 27;

pub struct Recorder {
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Option<cpal::Stream>,
    sample_rate: u32,
    max_samples: usize,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            sample_rate: 16_000,
            max_samples: 16_000 * MAX_RECORDING_SECONDS,
        }
    }

    pub fn start(
        &mut self,
        microphone: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.stream.is_some() {
            return Err("A recording is already in progress".into());
        }
        {
            let mut samples = self.samples.lock().map_err(|e| e.to_string())?;
            samples.clear();
        }

        let host = cpal::default_host();
        let devices = host.input_devices()?.collect::<Vec<_>>();
        let device = if microphone == "default" {
            host.default_input_device()
                .ok_or("No input device found. Check microphone permissions.")?
        } else {
            devices
                .into_iter()
                .find(|device| device.name().is_ok_and(|name| name == microphone))
                .ok_or_else(|| format!("Microphone '{microphone}' not found. Check Settings."))?
        };

        let default_config = device.default_input_config()?;
        let sample_format = default_config.sample_format();
        let config: cpal::StreamConfig = default_config.into();
        self.sample_rate = config.sample_rate.0;
        self.max_samples = self.sample_rate as usize * MAX_RECORDING_SECONDS;
        let channels = config.channels as usize;
        let samples = self.samples.clone();
        let max_samples = self.max_samples;

        let on_error = |error| log::error!("Audio stream error: {error}");
        let stream = match sample_format {
            cpal::SampleFormat::F32 => build_input_stream::<f32>(
                &device,
                &config,
                channels,
                samples,
                max_samples,
                on_error,
            )?,
            cpal::SampleFormat::I16 => build_input_stream::<i16>(
                &device,
                &config,
                channels,
                samples,
                max_samples,
                on_error,
            )?,
            cpal::SampleFormat::U16 => build_input_stream::<u16>(
                &device,
                &config,
                channels,
                samples,
                max_samples,
                on_error,
            )?,
            format => {
                return Err(format!("Unsupported microphone sample format: {format:?}").into())
            }
        };
        stream.play()?;
        self.stream = Some(stream);
        log::info!(
            "Recording started: {sample_format:?}, {}Hz, {channels} channel(s), max {}s",
            self.sample_rate,
            MAX_RECORDING_SECONDS
        );
        Ok(())
    }

    pub fn is_duration_limit_reached(&self) -> bool {
        self.samples
            .lock()
            .map(|samples| samples.len() >= self.max_samples)
            .unwrap_or(false)
    }

    pub fn stop(&mut self) -> Result<(Vec<f32>, u32), Box<dyn std::error::Error + Send + Sync>> {
        drop(self.stream.take());
        let samples = std::mem::take(&mut *self.samples.lock().map_err(|e| e.to_string())?);
        let sample_rate = self.sample_rate;
        log::info!(
            "Recording stopped: {:.1}s",
            samples.len() as f32 / sample_rate as f32
        );
        Ok((samples, sample_rate))
    }
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    samples: Arc<Mutex<Vec<f32>>>,
    max_samples: usize,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let Ok(mut output) = samples.lock() else {
                return;
            };
            let remaining = max_samples.saturating_sub(output.len());
            if remaining == 0 {
                return;
            }
            for chunk in data.chunks(channels).take(remaining) {
                let mono = chunk
                    .iter()
                    .map(|sample| f32::from_sample(*sample))
                    .sum::<f32>()
                    / channels as f32;
                output.push(mono.clamp(-1.0, 1.0));
            }
        },
        on_error,
        None,
    )
}

pub fn list_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| devices.filter_map(|device| device.name().ok()).collect())
        .unwrap_or_default()
}

/// Encode f32 samples as mono 16-bit PCM WAV at 16kHz.
pub fn encode_wav(
    samples: Vec<f32>,
    sample_rate: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let (samples, final_rate) = if sample_rate != 16_000 {
        (resample(&samples, sample_rate, 16_000), 16_000)
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
    for sample in samples {
        writer.write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
    }
    writer.finalize()?;
    Ok(cursor.into_inner())
}

fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() || from_rate == 0 || to_rate == 0 {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;
    (0..new_len)
        .map(|index| {
            let source_position = index as f64 * ratio;
            let source_index = source_position as usize;
            let fraction = (source_position - source_index as f64) as f32;
            if source_index + 1 < samples.len() {
                samples[source_index] * (1.0 - fraction) + samples[source_index + 1] * fraction
            } else {
                samples[source_index]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_48k_to_16k_has_expected_length() {
        let input = vec![0.25; 48_000];
        let output = resample(&input, 48_000, 16_000);
        assert_eq!(output.len(), 16_000);
        assert!(output.iter().all(|sample| (*sample - 0.25).abs() < 1e-6));
    }

    #[test]
    fn wav_encoder_writes_16k_mono_pcm() {
        let wav = encode_wav(vec![0.0; 48_000], 48_000).unwrap();
        let reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(reader.duration(), 16_000);
    }
}
