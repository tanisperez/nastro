//! Nastro's audio pipeline: our own chain from file to speakers, so the equalizer, the
//! visualizer and gapless playback stay under our control.
//!
//! # Pipeline
//!
//! ```text
//! file -> decode -> equalizer -> resample -> ring -> output stage -> device
//!         (engine thread, per chunk)            (device callback)      |
//!                                                                      +-> analysis tap
//! ```
//!
//! 1. **Decode**: a [`Decoder`] turns the file into interleaved `f32` chunks
//!    ([`SymphoniaDecoder`] is the real one; probing tags is cheap and never decodes).
//! 2. **Equalizer**: ten peaking filters and a preamp, at the sample rate of the source.
//! 3. **Resample**: a rubato sinc resampler when the source rate differs from the device
//!    rate, then the channel layout is converted to the device's (mono is copied to both
//!    sides, surround is downmixed, extra device channels are silent).
//! 4. **Ring**: a lock-free single producer ring of about 300 ms between the engine thread
//!    and the device callback, which is also the latency of seeks and of EQ changes.
//! 5. **Output stage**: the callback drains the ring, applies volume and balance with a short
//!    ramp, copies a mono mix into a small tap for the visualizer, and writes silence on an
//!    underrun. It never allocates, blocks, logs or waits for a lock.
//! 6. **Analysis**: the UI asks [`Player::analysis`] about 30 times a second for an
//!    [`AnalysisFrame`] computed from the tap.
//!
//! # Threads
//!
//! One engine thread owns the decoder stream, the equalizer, the resampler and the device
//! stream, and sleeps on its command channel (or briefly while the ring is full). Paused or
//! stopped, both the thread and the device are idle. The device callback runs on the audio
//! thread of the host. [`Player`] is a cheap, thread safe handle: its methods send commands
//! and return, and results come back as [`PlayerEvent`]s on an async channel, so the UI needs
//! no polling. Dropping the last handle shuts the engine thread down.

pub mod analysis;
pub mod decoder;
pub mod engine;
pub mod eq;
pub mod error;

#[cfg(test)]
mod test_util;

pub use analysis::{AnalysisFrame, Analyzer, SCOPE_POINTS, SPECTRUM_BARS};
pub use decoder::{
    DecodedStream, Decoder, Probe, SymphoniaDecoder, is_supported_path, supported_extensions,
};
pub use engine::{AudioSink, CpalSink, Player, PlayerEvent, PlayerStatus, Renderer, SinkFormat};
pub use eq::{Equalizer, response_db};
pub use error::AudioError;
