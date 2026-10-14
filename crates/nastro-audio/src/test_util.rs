//! Helpers shared by the tests: a tiny WAV writer and tone generators.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::f32::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

/// Interleaved samples of a sine, the same on every channel.
pub fn sine(rate: u32, channels: u16, seconds: f32, freq: f32, amplitude: f32) -> Vec<f32> {
    let frames = (rate as f32 * seconds) as usize;
    let mut out = Vec::with_capacity(frames * usize::from(channels));
    for n in 0..frames {
        let v = amplitude * (2.0 * PI * freq * n as f32 / rate as f32).sin();
        out.extend(std::iter::repeat_n(v, usize::from(channels)));
    }
    out
}

/// Writes `samples` (interleaved, `-1..=1`) as a 16 bit PCM WAV file.
pub fn write_wav(path: &Path, rate: u32, channels: u16, samples: &[f32]) {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(path, bytes).expect("write the test wav");
}

/// Writes a sine WAV named `name` into `dir` and returns its path.
pub fn sine_wav(dir: &Path, name: &str, rate: u32, channels: u16, seconds: f32) -> PathBuf {
    let path = dir.join(name);
    write_wav(
        &path,
        rate,
        channels,
        &sine(rate, channels, seconds, 440.0, 0.5),
    );
    path
}

/// Path of a committed fixture.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
