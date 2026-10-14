//! The per track processing between the decoder and the ring: equalizer, resampler and channel
//! layout conversion.

use nastro_core::EqSettings;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Indexing, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};

use crate::eq::Equalizer;
use crate::error::AudioError;

/// Input frames per resampler call.
const CHUNK_FRAMES: usize = 1024;

/// Rubato sinc resampler with a fixed ratio, the state around it, and what is needed to trim
/// its delay at the start and flush its tail at the end.
struct Resampling {
    resampler: Async<f32>,
    ratio: f64,
    /// Source frames not yet fed, interleaved.
    pending: Vec<f32>,
    out: Vec<f32>,
    /// Frames of start-up delay still to drop from the output.
    skip: usize,
    in_total: u64,
    out_total: u64,
}

/// Equalizer, resampler and channel conversion for one stream, producing frames in the
/// layout of the device.
pub(crate) struct Pipeline {
    src_rate: u32,
    src_channels: usize,
    dst_channels: usize,
    eq: Equalizer,
    resampling: Option<Resampling>,
}

impl Pipeline {
    /// A pipeline from the source format to the device format.
    pub fn new(
        src_rate: u32,
        src_channels: u16,
        dst_rate: u32,
        dst_channels: u16,
        eq: &EqSettings,
    ) -> Result<Self, AudioError> {
        let channels = usize::from(src_channels.max(1));
        let mut equalizer = Equalizer::new(src_rate, src_channels);
        equalizer.set(eq);
        let resampling = if src_rate == dst_rate {
            None
        } else {
            let ratio = f64::from(dst_rate) / f64::from(src_rate);
            let params = SincInterpolationParameters::new(128, WindowFunction::BlackmanHarris2)
                .interpolation(SincInterpolationType::Linear)
                .oversampling_factor(256);
            let resampler = Async::<f32>::new_sinc(
                ratio,
                1.0,
                &params,
                CHUNK_FRAMES,
                channels,
                FixedAsync::Input,
            )
            .map_err(|err| AudioError::Decode(format!("cannot create the resampler: {err}")))?;
            let out = vec![0.0; resampler.output_frames_max() * channels];
            let skip = resampler.output_delay();
            Some(Resampling {
                resampler,
                ratio,
                pending: Vec::new(),
                out,
                skip,
                in_total: 0,
                out_total: 0,
            })
        };
        Ok(Pipeline {
            src_rate,
            src_channels: channels,
            dst_channels: usize::from(dst_channels.max(1)),
            eq: equalizer,
            resampling,
        })
    }

    /// Whether the pipeline can carry a stream with this format without being rebuilt.
    pub fn fits(&self, src_rate: u32, src_channels: u16) -> bool {
        self.src_rate == src_rate && self.src_channels == usize::from(src_channels.max(1))
    }

    /// Applies new equalizer settings.
    pub fn set_eq(&mut self, settings: &EqSettings) {
        self.eq.set(settings);
    }

    /// Runs `samples` (interleaved, source layout) through the pipeline and appends device
    /// frames to `out`.
    pub fn process(&mut self, samples: &mut [f32], out: &mut Vec<f32>) {
        self.eq.process(samples);
        match &mut self.resampling {
            None => convert_channels(samples, self.src_channels, self.dst_channels, out),
            Some(r) => {
                r.pending.extend_from_slice(samples);
                r.run(false, self.src_channels, self.dst_channels, out);
            }
        }
    }

    /// Pushes out what the resampler still holds, at the end of a stream.
    pub fn flush(&mut self, out: &mut Vec<f32>) {
        if let Some(r) = &mut self.resampling {
            r.run(true, self.src_channels, self.dst_channels, out);
            r.reset();
        }
    }
}

impl Resampling {
    fn reset(&mut self) {
        self.resampler.reset();
        self.pending.clear();
        self.skip = self.resampler.output_delay();
        self.in_total = 0;
        self.out_total = 0;
    }

    /// Feeds every complete chunk, and with `flush` the partial rest and the tail too.
    fn run(&mut self, flush: bool, src_ch: usize, dst_ch: usize, out: &mut Vec<f32>) {
        let mut tail_calls = 0;
        loop {
            let need = self.resampler.input_frames_next();
            let available = self.pending.len() / src_ch;
            let partial = if available >= need {
                None
            } else if flush && tail_calls < 8 {
                tail_calls += 1;
                Some(available)
            } else {
                return;
            };
            if partial.is_some() {
                self.pending.resize(need * src_ch, 0.0);
            }
            let mut indexing = Indexing::new();
            if let Some(len) = partial {
                indexing = indexing.partial_len(len);
            }
            let capacity = self.out.len() / src_ch;
            let result = {
                let Ok(input) = InterleavedSlice::new(&self.pending[..need * src_ch], src_ch, need)
                else {
                    return;
                };
                let Ok(mut output) = InterleavedSlice::new_mut(&mut self.out, src_ch, capacity)
                else {
                    return;
                };
                self.resampler
                    .process_into_buffer(&input, &mut output, Some(&indexing))
            };
            let Ok((consumed, produced)) = result else {
                self.pending.clear();
                return;
            };
            self.in_total += partial.unwrap_or(consumed) as u64;
            let skipped = self.skip.min(produced);
            self.skip -= skipped;
            let mut keep = produced - skipped;
            let wanted = (self.in_total as f64 * self.ratio).ceil() as u64;
            if partial.is_some() {
                // At the end only the frames that correspond to real input are wanted.
                keep = keep.min(wanted.saturating_sub(self.out_total) as usize);
            }
            self.out_total += keep as u64;
            let frames = &self.out[skipped * src_ch..(skipped + keep) * src_ch];
            convert_channels(frames, src_ch, dst_ch, out);
            if partial.is_some() {
                self.pending.clear();
                if self.out_total >= wanted {
                    return;
                }
            } else {
                self.pending.drain(..consumed * src_ch);
            }
        }
    }
}

/// Appends `src` (interleaved with `from` channels) to `out` with `to` channels per frame.
/// Mono is copied to both sides, more than two channels are downmixed to stereo, and device
/// channels beyond the second get silence.
pub(crate) fn convert_channels(src: &[f32], from: usize, to: usize, out: &mut Vec<f32>) {
    if from == to {
        out.extend_from_slice(src);
        return;
    }
    for frame in src.chunks_exact(from) {
        let (left, right) = match from {
            1 => (frame[0], frame[0]),
            2 => (frame[0], frame[1]),
            _ => {
                let centre = frame[2] * 0.707;
                let (rear_l, rear_r) = if from >= 6 {
                    (frame[4] * 0.707, frame[5] * 0.707)
                } else {
                    (0.0, 0.0)
                };
                (
                    (frame[0] + centre + rear_l) / 2.414,
                    (frame[1] + centre + rear_r) / 2.414,
                )
            }
        };
        if to == 1 {
            out.push((left + right) * 0.5);
        } else {
            out.push(left);
            out.push(right);
            out.extend(std::iter::repeat_n(0.0, to - 2));
        }
    }
}
