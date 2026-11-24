use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample, StreamConfig};
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use thiserror::Error;

const SOURCE_SAMPLE_RATE: f64 = 16_000.0;
const AUDIO_QUEUE_CHUNKS: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("no audio output device is available")]
    NoOutputDevice,
    #[error("audio output device '{name}' was not found")]
    DeviceNotFound { name: String },
    #[error("audio device is not available: {0}")]
    DeviceUnavailable(String),
    #[error("audio format {0} is not supported by this output device")]
    UnsupportedFormat(String),
    #[error("audio stream failed: {0}")]
    Stream(String),
    #[error("audio worker failed to start")]
    WorkerStart,
    #[error("audio worker stopped unexpectedly")]
    WorkerStopped,
}

pub fn list_output_devices() -> Result<Vec<AudioDevice>, AudioError> {
    list_devices(true)
}

pub fn list_input_devices() -> Result<Vec<AudioDevice>, AudioError> {
    list_devices(false)
}

fn list_devices(output: bool) -> Result<Vec<AudioDevice>, AudioError> {
    let host = cpal::default_host();
    let default_name = if output {
        host.default_output_device()
            .map(|device| device_name(&device))
    } else {
        host.default_input_device()
            .map(|device| device_name(&device))
    };
    let devices = if output {
        host.output_devices()
    } else {
        host.input_devices()
    }
    .map_err(|error| AudioError::DeviceUnavailable(error.to_string()))?;
    let mut result = devices
        .map(|device| {
            let name = device_name(&device);
            AudioDevice {
                is_default: default_name.as_ref() == Some(&name),
                name,
            }
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(result)
}

pub fn open_output(preferred_name: &str, source_gain: f32) -> Result<AudioOutput, AudioError> {
    let gain = Arc::new(AtomicU32::new(source_gain.clamp(0.0, 16.0).to_bits()));
    open_output_with_gain(preferred_name, gain)
}

pub fn open_output_with_gain(
    preferred_name: &str,
    gain: Arc<AtomicU32>,
) -> Result<AudioOutput, AudioError> {
    let (ready_tx, ready_rx) = bounded(1);
    let (samples_tx, samples_rx) = bounded(AUDIO_QUEUE_CHUNKS);
    let (stop_tx, stop_rx) = bounded(1);
    let preferred = preferred_name.trim().to_string();
    let worker_gain = Arc::clone(&gain);

    let worker = std::thread::Builder::new()
        .name("remote-buddy-audio-output".to_string())
        .spawn(move || {
            let stream = build_stream(&preferred, samples_rx, worker_gain);
            match stream {
                Ok(stream) => {
                    let _ = ready_tx.send(Ok(()));
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                }
            }
        })
        .map_err(|_| AudioError::WorkerStart)?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(AudioOutput {
            samples: samples_tx,
            stop: stop_tx,
            gain,
            worker: Some(worker),
        }),
        Ok(Err(error)) => {
            let _ = worker.join();
            Err(error)
        }
        Err(_) => {
            let _ = worker.join();
            Err(AudioError::WorkerStopped)
        }
    }
}

pub struct AudioOutput {
    samples: Sender<Vec<f32>>,
    stop: Sender<()>,
    gain: Arc<AtomicU32>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for AudioOutput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioOutput")
            .field("gain", &self.gain())
            .field("queued_chunks", &self.samples.len())
            .finish_non_exhaustive()
    }
}

impl AudioOutput {
    pub fn push_pcm16(&self, samples: &[i16]) {
        if samples.is_empty() {
            return;
        }
        let converted = samples
            .iter()
            .map(|sample| f32::from(*sample) / 32768.0)
            .collect();
        match self.samples.try_send(converted) {
            Ok(()) | Err(TrySendError::Disconnected(_)) | Err(TrySendError::Full(_)) => {}
        }
    }

    pub fn set_gain(&self, gain: f32) {
        self.gain
            .store(gain.clamp(0.0, 16.0).to_bits(), Ordering::Relaxed);
    }

    pub fn gain(&self) -> f32 {
        f32::from_bits(self.gain.load(Ordering::Relaxed))
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn build_stream(
    preferred_name: &str,
    samples: Receiver<Vec<f32>>,
    gain: Arc<AtomicU32>,
) -> Result<cpal::Stream, AudioError> {
    let host = cpal::default_host();
    let device = select_device(&host, preferred_name)?;
    let supported = device
        .default_output_config()
        .map_err(|error| AudioError::DeviceUnavailable(error.to_string()))?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let channels = usize::from(config.channels);
    let output_rate = f64::from(config.sample_rate);
    let error_callback = |error: cpal::Error| {
        tracing::error!(%error, "audio output stream error");
    };

    let stream = match sample_format {
        SampleFormat::F32 => build_typed_stream::<f32>(
            &device,
            config,
            channels,
            output_rate,
            samples,
            gain,
            error_callback,
        ),
        SampleFormat::I16 => build_typed_stream::<i16>(
            &device,
            config,
            channels,
            output_rate,
            samples,
            gain,
            error_callback,
        ),
        SampleFormat::U16 => build_typed_stream::<u16>(
            &device,
            config,
            channels,
            output_rate,
            samples,
            gain,
            error_callback,
        ),
        SampleFormat::I32 => build_typed_stream::<i32>(
            &device,
            config,
            channels,
            output_rate,
            samples,
            gain,
            error_callback,
        ),
        SampleFormat::I24 => build_typed_stream::<cpal::I24>(
            &device,
            config,
            channels,
            output_rate,
            samples,
            gain,
            error_callback,
        ),
        SampleFormat::U32 => build_typed_stream::<u32>(
            &device,
            config,
            channels,
            output_rate,
            samples,
            gain,
            error_callback,
        ),
        SampleFormat::U24 => build_typed_stream::<cpal::U24>(
            &device,
            config,
            channels,
            output_rate,
            samples,
            gain,
            error_callback,
        ),
        other => return Err(AudioError::UnsupportedFormat(other.to_string())),
    }?;

    stream
        .play()
        .map_err(|error| AudioError::Stream(error.to_string()))?;
    Ok(stream)
}

fn build_typed_stream<T>(
    device: &cpal::Device,
    config: StreamConfig,
    channels: usize,
    output_rate: f64,
    samples: Receiver<Vec<f32>>,
    gain: Arc<AtomicU32>,
    error_callback: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample + cpal::FromSample<f32>,
{
    let mut source = SourceQueue::new(samples);
    let mut resampler = LinearResampler::new(output_rate);
    let gain = Arc::clone(&gain);
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                let gain = f32::from_bits(gain.load(Ordering::Relaxed));
                for frame in output.chunks_mut(channels) {
                    let sample = (resampler.next(&mut source) * gain).clamp(-1.0, 1.0);
                    let value = T::from_sample(sample);
                    for channel in frame {
                        *channel = value;
                    }
                }
            },
            error_callback,
            Some(Duration::from_millis(500)),
        )
        .map_err(|error| AudioError::Stream(error.to_string()))
}

fn select_device(host: &cpal::Host, preferred_name: &str) -> Result<cpal::Device, AudioError> {
    let devices = host
        .output_devices()
        .map_err(|error| AudioError::DeviceUnavailable(error.to_string()))?
        .collect::<Vec<_>>();

    if preferred_name.is_empty() || preferred_name.eq_ignore_ascii_case("default") {
        return host
            .default_output_device()
            .ok_or(AudioError::NoOutputDevice);
    }

    let preferred_lower = preferred_name.to_ascii_lowercase();
    if let Some(device) = devices.iter().find(|device| {
        device_name(device)
            .to_ascii_lowercase()
            .contains(&preferred_lower)
    }) {
        return Ok(device.clone());
    }

    if preferred_lower.contains("cable")
        && let Some(device) = devices.iter().find(|device| {
            let name = device_name(device).to_ascii_lowercase();
            name.contains("vb-audio") || name.contains("virtual cable")
        })
    {
        return Ok(device.clone());
    }

    Err(AudioError::DeviceNotFound {
        name: preferred_name.to_string(),
    })
}

fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|description| description.name().to_string())
        .unwrap_or_else(|_| device.to_string())
}

struct SourceQueue {
    receiver: Receiver<Vec<f32>>,
    current: std::vec::IntoIter<f32>,
}

impl SourceQueue {
    fn new(receiver: Receiver<Vec<f32>>) -> Self {
        Self {
            receiver,
            current: Vec::new().into_iter(),
        }
    }

    fn next(&mut self) -> f32 {
        loop {
            if let Some(sample) = self.current.next() {
                return sample;
            }
            match self.receiver.try_recv() {
                Ok(chunk) => self.current = chunk.into_iter(),
                Err(_) => return 0.0,
            }
        }
    }
}

struct LinearResampler {
    output_rate: f64,
    phase: f64,
    previous: f32,
    current: Option<f32>,
}

impl LinearResampler {
    fn new(output_rate: f64) -> Self {
        Self {
            output_rate: output_rate.max(1.0),
            phase: 0.0,
            previous: 0.0,
            current: None,
        }
    }

    fn next(&mut self, source: &mut SourceQueue) -> f32 {
        if self.current.is_none() {
            self.previous = source.next();
            self.current = Some(self.previous);
        }
        let next = self.current.unwrap_or_default();
        let value = self.previous + (next - self.previous) * self.phase as f32;
        self.phase += SOURCE_SAMPLE_RATE / self.output_rate;
        while self.phase >= 1.0 {
            self.previous = next;
            let next = source.next();
            self.current = Some(next);
            self.phase -= 1.0;
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_upsamples_a_constant_signal() {
        let (_tx, rx) = bounded(1);
        let mut queue = SourceQueue::new(rx);
        queue.current = vec![0.5_f32; 480].into_iter();
        let mut resampler = LinearResampler::new(48_000.0);
        for _ in 0..960 {
            let sample = resampler.next(&mut queue);
            assert!((sample - 0.5).abs() < 0.000_1);
        }
    }

    #[test]
    fn audio_output_uses_the_shared_gain_atomic() {
        let gain = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let (samples, _samples_rx) = bounded(1);
        let (stop, _stop_rx) = bounded(1);
        let output = AudioOutput {
            samples,
            stop,
            gain: Arc::clone(&gain),
            worker: None,
        };

        gain.store(2.5_f32.to_bits(), Ordering::Relaxed);
        assert_eq!(output.gain(), 2.5);

        output.set_gain(3.0);
        assert_eq!(f32::from_bits(gain.load(Ordering::Relaxed)), 3.0);
    }
}
