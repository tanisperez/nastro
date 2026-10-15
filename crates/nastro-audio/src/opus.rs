//! Opus decoding. Symphonia demuxes Opus from Ogg, Matroska and WebM but has no decoder for
//! it, so [`OpusAudioDecoder`] plugs the pure Rust `opus-decoder` crate into Symphonia's codec
//! registry. The containers, seeking and the rest of [`crate::SymphoniaDecoder`] stay the same.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::LazyLock;

use symphonia::core::audio::{
    AsGenericAudioBufferRef, AudioBuffer, AudioMut, AudioSpec, Channels, GenericAudioBufferRef,
};
use symphonia::core::codecs::CodecInfo;
use symphonia::core::codecs::audio::well_known::CODEC_ID_OPUS;
use symphonia::core::codecs::audio::{
    AudioCodecParameters, AudioDecoder, AudioDecoderOptions, FinalizeResult,
};
use symphonia::core::codecs::registry::{
    CodecRegistry, RegisterableAudioDecoder, SupportedAudioCodec,
};
use symphonia::core::errors::{Error, Result};
use symphonia::core::packet::PacketRef;

/// Opus always decodes at 48 kHz, whatever the rate of the original audio was.
const SAMPLE_RATE: u32 = 48_000;

/// The longest Opus packet: 120 ms at 48 kHz, in frames.
const MAX_PACKET_FRAMES: usize = 5_760;

/// Symphonia's codecs plus ours.
static CODECS: LazyLock<CodecRegistry> = LazyLock::new(|| {
    let mut registry = CodecRegistry::new();
    symphonia::default::register_enabled_codecs(&mut registry);
    registry.register_audio_decoder::<OpusAudioDecoder>();
    registry
});

/// The codec registry the pipeline makes its decoders from.
pub(crate) fn codecs() -> &'static CodecRegistry {
    &CODECS
}

/// What the `OpusHead` identification header (RFC 7845, section 5.1) says.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OpusHead {
    channels: usize,
    /// Frames to drop at the start of the stream, at 48 kHz.
    pre_skip: usize,
    /// Output gain in Q7.8 dB.
    gain: i16,
    family: u8,
    streams: usize,
    coupled: usize,
    /// For each output channel, the decoded channel it takes, or 255 for silence.
    mapping: Vec<u8>,
}

/// Parses an `OpusHead` header, as Ogg stores it in its first packet and Matroska in the
/// codec private data.
fn parse_head(data: &[u8]) -> Result<OpusHead> {
    let malformed = || Error::DecodeError("opus: malformed OpusHead");
    if data.len() < 19 || &data[..8] != b"OpusHead" {
        return Err(malformed());
    }
    // Versions 0 to 15 share this layout.
    if data[8] > 15 {
        return Err(Error::Unsupported("opus: OpusHead version"));
    }
    let channels = usize::from(data[9]);
    let pre_skip = usize::from(u16::from_le_bytes([data[10], data[11]]));
    let gain = i16::from_le_bytes([data[16], data[17]]);
    let family = data[18];
    if channels == 0 {
        return Err(malformed());
    }
    let (streams, coupled, mapping) = if family == 0 {
        if channels > 2 {
            return Err(malformed());
        }
        (1, channels - 1, (0..channels as u8).collect())
    } else {
        let table = data.get(19..21 + channels).ok_or_else(malformed)?;
        let (streams, coupled) = (usize::from(table[0]), usize::from(table[1]));
        if streams == 0 || coupled > streams {
            return Err(malformed());
        }
        (streams, coupled, table[2..].to_vec())
    };
    Ok(OpusHead {
        channels,
        pre_skip,
        gain,
        family,
        streams,
        coupled,
        mapping,
    })
}

/// The plane Symphonia expects for decoded channel `ch` of `channels`. Mapping family 1 uses
/// the Vorbis channel order, Symphonia orders planes by speaker position.
fn plane_of(family: u8, channels: usize, ch: usize) -> usize {
    if family != 1 {
        return ch;
    }
    let order: &[usize] = match channels {
        3 => &[0, 2, 1],
        5 => &[0, 2, 1, 3, 4],
        6 => &[0, 2, 1, 4, 5, 3],
        7 => &[0, 2, 1, 5, 6, 4, 3],
        8 => &[0, 2, 1, 6, 7, 4, 5, 3],
        _ => return ch,
    };
    order[ch]
}

/// The `opus-decoder` decoder for one stream, or for several when there are more than two
/// channels.
enum Inner {
    Single(Box<opus_decoder::OpusDecoder>),
    Multi(opus_decoder::OpusMultistreamDecoder),
}

impl Inner {
    fn new(head: &OpusHead) -> std::result::Result<Self, opus_decoder::OpusError> {
        let identity = head
            .mapping
            .iter()
            .enumerate()
            .all(|(i, m)| usize::from(*m) == i);
        if head.streams == 1 && identity {
            let decoder = opus_decoder::OpusDecoder::new(SAMPLE_RATE, head.channels)?;
            Ok(Self::Single(Box::new(decoder)))
        } else {
            let decoder = opus_decoder::OpusMultistreamDecoder::new(
                SAMPLE_RATE,
                head.channels,
                head.streams,
                head.coupled,
                &head.mapping,
            )?;
            Ok(Self::Multi(decoder))
        }
    }

    /// Decodes one packet into interleaved samples, returning the frames written.
    fn decode(
        &mut self,
        packet: &[u8],
        pcm: &mut [f32],
    ) -> std::result::Result<usize, opus_decoder::OpusError> {
        match self {
            Self::Single(decoder) => decoder.decode_float(packet, pcm, false),
            Self::Multi(decoder) => decoder.decode_float(packet, pcm, false),
        }
    }

    fn reset(&mut self) {
        match self {
            Self::Single(decoder) => decoder.reset(),
            Self::Multi(decoder) => decoder.reset(),
        }
    }
}

/// A Symphonia [`AudioDecoder`] for Opus.
pub(crate) struct OpusAudioDecoder {
    params: AudioCodecParameters,
    gapless: bool,
    head: OpusHead,
    inner: Inner,
    /// Linear factor of the header's output gain.
    gain: f32,
    /// Whether the next packet is the first since the decoder was made or reset.
    fresh: bool,
    /// Frames still to drop at the start of the stream.
    pending_skip: usize,
    pcm: Vec<f32>,
    buf: AudioBuffer<f32>,
}

impl OpusAudioDecoder {
    fn try_new(params: &AudioCodecParameters, opts: &AudioDecoderOptions) -> Result<Self> {
        let extra = params
            .extra_data
            .as_deref()
            .ok_or(Error::DecodeError("opus: no OpusHead"))?;
        let head = parse_head(extra)?;
        let inner = Inner::new(&head).map_err(|err| {
            log::debug!("opus: cannot make the decoder: {err}");
            Error::Unsupported("opus: channel layout")
        })?;
        let channels = match &params.channels {
            Some(channels) if channels.count() == head.channels => channels.clone(),
            _ => Channels::Discrete(head.channels as u16),
        };
        let mut params = params.clone();
        params
            .with_sample_rate(SAMPLE_RATE)
            .with_channels(channels.clone());
        let gain = 10f32.powf(f32::from(head.gain) / (256.0 * 20.0));
        Ok(Self {
            params,
            gapless: opts.gapless,
            pcm: vec![0.0; MAX_PACKET_FRAMES * head.channels],
            buf: AudioBuffer::new(AudioSpec::new(SAMPLE_RATE, channels), MAX_PACKET_FRAMES),
            head,
            inner,
            gain,
            fresh: true,
            pending_skip: 0,
        })
    }

    fn decode_inner(&mut self, packet: &PacketRef<'_>) -> Result<()> {
        self.buf.clear();
        let fresh = std::mem::replace(&mut self.fresh, false);
        // Neither Ogg nor Matroska trims the pre-skip. The first packet of a stream starts at
        // zero in Ogg and below zero in Matroska (shifted back by the codec delay), while a
        // packet after a seek starts later.
        if fresh && self.gapless && packet.pts.get() <= 0 {
            self.pending_skip = self.head.pre_skip;
        }
        let (inner, pcm) = (&mut self.inner, &mut self.pcm);
        // `opus-decoder` is young: a panic on a damaged packet must not take the engine down.
        let decoded = catch_unwind(AssertUnwindSafe(|| inner.decode(packet.data, pcm)));
        let frames = match decoded {
            Ok(Ok(frames)) => frames.min(MAX_PACKET_FRAMES),
            Ok(Err(err)) => {
                log::debug!("opus: {err}");
                return Err(Error::DecodeError("opus: invalid packet"));
            }
            Err(_) => {
                self.inner.reset();
                return Err(Error::DecodeError("opus: decoder failure"));
            }
        };
        let channels = self.head.channels;
        self.buf.resize_uninit(frames);
        for ch in 0..channels {
            let plane_index = plane_of(self.head.family, channels, ch);
            let Some(plane) = self.buf.plane_mut(plane_index) else {
                return Err(Error::DecodeError("opus: missing audio plane"));
            };
            let samples = self.pcm.iter().skip(ch).step_by(channels);
            for (out, sample) in plane[..frames].iter_mut().zip(samples) {
                *out = sample * self.gain;
            }
        }
        if self.gapless {
            let skip = self.pending_skip.min(frames);
            self.pending_skip -= skip;
            let start = (packet.trim_start.get() as usize).max(skip);
            self.buf.trim(start, packet.trim_end.get() as usize);
        }
        Ok(())
    }
}

impl AudioDecoder for OpusAudioDecoder {
    fn reset(&mut self) {
        self.inner.reset();
        self.fresh = true;
        self.pending_skip = 0;
    }

    fn codec_info(&self) -> &CodecInfo {
        &SUPPORTED[0].info
    }

    fn codec_params(&self) -> &AudioCodecParameters {
        &self.params
    }

    fn decode_ref(&mut self, packet: &PacketRef<'_>) -> Result<GenericAudioBufferRef<'_>> {
        if let Err(err) = self.decode_inner(packet) {
            self.buf.clear();
            return Err(err);
        }
        Ok(self.buf.as_generic_audio_buffer_ref())
    }

    fn finalize(&mut self) -> FinalizeResult {
        FinalizeResult::default()
    }

    fn last_decoded(&self) -> GenericAudioBufferRef<'_> {
        self.buf.as_generic_audio_buffer_ref()
    }
}

/// The one codec this decoder handles.
const SUPPORTED: &[SupportedAudioCodec] = &[SupportedAudioCodec {
    id: CODEC_ID_OPUS,
    info: CodecInfo {
        short_name: "opus",
        long_name: "Opus",
        profiles: &[],
    },
}];

impl RegisterableAudioDecoder for OpusAudioDecoder {
    fn try_registry_new(
        params: &AudioCodecParameters,
        opts: &AudioDecoderOptions,
    ) -> Result<Box<dyn AudioDecoder>>
    where
        Self: Sized,
    {
        Ok(Box::new(Self::try_new(params, opts)?))
    }

    fn supported_codecs() -> &'static [SupportedAudioCodec] {
        SUPPORTED
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    /// An `OpusHead` with the given channel count, pre-skip, gain and mapping family.
    fn head(channels: u8, pre_skip: u16, gain: i16, family: u8, table: &[u8]) -> Vec<u8> {
        let mut data = b"OpusHead".to_vec();
        data.extend_from_slice(&[1, channels]);
        data.extend_from_slice(&pre_skip.to_le_bytes());
        data.extend_from_slice(&44_100u32.to_le_bytes());
        data.extend_from_slice(&gain.to_le_bytes());
        data.push(family);
        data.extend_from_slice(table);
        data
    }

    #[test]
    fn family_zero_heads_are_one_stream() {
        let parsed = parse_head(&head(2, 312, -256, 0, &[])).unwrap();
        assert_eq!(
            parsed,
            OpusHead {
                channels: 2,
                pre_skip: 312,
                gain: -256,
                family: 0,
                streams: 1,
                coupled: 1,
                mapping: vec![0, 1],
            }
        );
        assert!(parse_head(&head(3, 0, 0, 0, &[])).is_err());
    }

    #[test]
    fn family_one_heads_carry_the_mapping_table() {
        let table = [4, 2, 0, 4, 1, 2, 3, 5];
        let parsed = parse_head(&head(6, 0, 0, 1, &table)).unwrap();
        assert_eq!((parsed.streams, parsed.coupled), (4, 2));
        assert_eq!(parsed.mapping, vec![0, 4, 1, 2, 3, 5]);
        // A table shorter than the channel count is rejected.
        assert!(parse_head(&head(6, 0, 0, 1, &table[..5])).is_err());
    }

    #[test]
    fn bad_heads_are_errors() {
        assert!(parse_head(b"OpusHead").is_err());
        assert!(parse_head(&head(0, 0, 0, 0, &[])).is_err());
        let mut wrong_magic = head(2, 0, 0, 0, &[]);
        wrong_magic[0] = b'X';
        assert!(parse_head(&wrong_magic).is_err());
    }

    #[test]
    fn vorbis_order_maps_to_symphonia_planes() {
        // 5.1 in Vorbis order: FL, FC, FR, RL, RR, LFE.
        let planes: Vec<usize> = (0..6).map(|ch| plane_of(1, 6, ch)).collect();
        assert_eq!(planes, vec![0, 2, 1, 4, 5, 3]);
        assert_eq!(plane_of(0, 2, 1), 1);
        assert_eq!(plane_of(255, 6, 1), 1);
    }
}
