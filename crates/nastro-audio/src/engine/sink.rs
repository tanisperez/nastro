//! Where the audio goes: the [`AudioSink`] trait and the cpal implementation.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig};

use super::render::Renderer;
use crate::error::AudioError;

/// The format of the device, which the pipeline converts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkFormat {
    /// Device sample rate, in Hz.
    pub sample_rate: u32,
    /// Device channel count.
    pub channels: u16,
}

/// An audio output. It lives on the engine thread, so it does not need to be `Send`.
///
/// The device is only touched when the engine first needs it, so a machine without audio
/// output can still run the app.
pub trait AudioSink {
    /// The format the device wants, without starting anything.
    fn format(&mut self) -> Result<SinkFormat, AudioError>;

    /// Creates the stream, paused, driven by `renderer`.
    fn open(&mut self, renderer: Renderer) -> Result<(), AudioError>;

    /// Starts calling the renderer.
    fn resume(&mut self) -> Result<(), AudioError>;

    /// Stops calling the renderer, letting the device sleep.
    fn pause(&mut self) -> Result<(), AudioError>;
}

/// Frames of scratch space for devices that do not take `f32`.
const CONVERT_FRAMES: usize = 4096;

/// The default output device through cpal.
#[derive(Default)]
pub struct CpalSink {
    device: Option<(Device, StreamConfig, SampleFormat)>,
    stream: Option<Stream>,
}

impl CpalSink {
    /// A sink that has not touched the device yet.
    pub fn new() -> Self {
        CpalSink::default()
    }

    fn device(&mut self) -> Result<&(Device, StreamConfig, SampleFormat), AudioError> {
        if self.device.is_none() {
            let device = cpal::default_host()
                .default_output_device()
                .ok_or_else(|| AudioError::Device("no output device".into()))?;
            let supported = device
                .default_output_config()
                .map_err(|err| AudioError::Device(err.to_string()))?;
            let format = supported.sample_format();
            self.device = Some((device, supported.into(), format));
        }
        self.device
            .as_ref()
            .ok_or_else(|| AudioError::Device("no output device".into()))
    }
}

/// Builds a stream for samples of type `T`, converting from the renderer's `f32`.
fn build<T>(
    device: &Device,
    config: StreamConfig,
    mut renderer: Renderer,
) -> Result<Stream, AudioError>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let channels = usize::from(config.channels.max(1));
    let mut scratch = vec![0.0f32; CONVERT_FRAMES * channels];
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                for part in data.chunks_mut(scratch.len()) {
                    let buffer = &mut scratch[..part.len()];
                    renderer.render(buffer);
                    for (dst, src) in part.iter_mut().zip(buffer.iter()) {
                        *dst = T::from_sample(*src);
                    }
                }
            },
            |err| log::warn!("audio stream error: {err}"),
            None,
        )
        .map_err(|err| AudioError::Device(err.to_string()))
}

impl AudioSink for CpalSink {
    fn format(&mut self) -> Result<SinkFormat, AudioError> {
        let (_, config, _) = self.device()?;
        Ok(SinkFormat {
            sample_rate: config.sample_rate,
            channels: config.channels,
        })
    }

    fn open(&mut self, mut renderer: Renderer) -> Result<(), AudioError> {
        let (device, config, format) = self.device()?;
        let config = *config;
        let stream = match format {
            SampleFormat::F32 => device
                .build_output_stream(
                    config,
                    move |data: &mut [f32], _| {
                        renderer.render(data);
                    },
                    |err| log::warn!("audio stream error: {err}"),
                    None,
                )
                .map_err(|err| AudioError::Device(err.to_string()))?,
            SampleFormat::I16 => build::<i16>(device, config, renderer)?,
            SampleFormat::U16 => build::<u16>(device, config, renderer)?,
            SampleFormat::I32 => build::<i32>(device, config, renderer)?,
            SampleFormat::U8 => build::<u8>(device, config, renderer)?,
            SampleFormat::F64 => build::<f64>(device, config, renderer)?,
            other => {
                return Err(AudioError::Device(format!(
                    "unsupported sample format {other}"
                )));
            }
        };
        // Some hosts start a stream as soon as it is built, others (ALSA) refuse to pause one
        // that never ran. Either way it ends up not playing, so a refusal is harmless.
        if let Err(err) = stream.pause() {
            log::debug!("pausing the new stream: {err}");
        }
        self.stream = Some(stream);
        Ok(())
    }

    fn resume(&mut self) -> Result<(), AudioError> {
        match &self.stream {
            Some(stream) => stream
                .play()
                .map_err(|err| AudioError::Device(err.to_string())),
            None => Ok(()),
        }
    }

    fn pause(&mut self) -> Result<(), AudioError> {
        match &self.stream {
            Some(stream) => stream
                .pause()
                .map_err(|err| AudioError::Device(err.to_string())),
            None => Ok(()),
        }
    }
}
