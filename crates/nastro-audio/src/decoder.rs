//! Decoding files into interleaved `f32` samples, and cheap metadata probing.
//!
//! The [`Decoder`] and [`DecodedStream`] traits keep the engine independent of the codec
//! library, so tests can feed it anything. [`SymphoniaDecoder`] is the real implementation.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use nastro_core::{StreamInfo, TrackMeta};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::{
    AudioCodecId, AudioCodecParameters, AudioDecoder, AudioDecoderOptions,
};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTag};
use symphonia::core::units::{Time, TimeBase, Timestamp};

use crate::error::AudioError;

/// File extensions (lowercase, no dot) the decoder can handle.
const EXTENSIONS: &[&str] = &[
    "mp1", "mp2", "mp3", "flac", "ogg", "oga", "wav", "wave", "aif", "aiff", "aifc", "m4a", "mp4",
    "m4b", "aac", "caf", "mka", "mkv", "webm",
];

/// The file extensions the audio pipeline can play (lowercase, without the dot).
pub fn supported_extensions() -> &'static [&'static str] {
    EXTENSIONS
}

/// Whether `path` has an extension of a supported audio format, compared case-insensitively.
pub fn is_supported_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// What a cheap look at a file tells: its tags, length and technical details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// Tags and duration.
    pub meta: TrackMeta,
    /// Codec, bitrate, sample rate and channels.
    pub info: StreamInfo,
}

/// Opens audio files. Implementations must be shareable between threads.
pub trait Decoder: Send + Sync {
    /// Opens `path` for decoding from the start.
    fn open(&self, path: &Path) -> Result<Box<dyn DecodedStream>, AudioError>;

    /// Reads the metadata and the codec parameters of `path` without decoding it.
    fn probe(&self, path: &Path) -> Result<Probe, AudioError>;
}

/// A file being decoded into interleaved `f32` samples.
pub trait DecodedStream: Send {
    /// Sample rate of the decoded samples, in Hz.
    fn sample_rate(&self) -> u32;

    /// Number of interleaved channels.
    fn channels(&self) -> u16;

    /// Technical details of the stream, for the display.
    fn info(&self) -> &StreamInfo;

    /// Total length, when the container says.
    fn duration(&self) -> Option<Duration>;

    /// Appends the next chunk of interleaved samples in `[-1, 1]` to `out`. Returns `Ok(false)`
    /// at the end of the stream, with nothing appended. Isolated corrupt packets are skipped.
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError>;

    /// Moves to `position` and returns the position actually reached, which is exact for
    /// the formats that allow sample accurate seeking.
    fn seek(&mut self, position: Duration) -> Result<Duration, AudioError>;
}

/// The [`Decoder`] backed by Symphonia, with gapless trimming enabled.
#[derive(Debug, Default, Clone, Copy)]
pub struct SymphoniaDecoder;

/// Seeks to a position below this reopen the file instead of using the container's seek.
const REWIND_BELOW: Duration = Duration::from_millis(200);

/// A probed container: the reader plus what was found about its audio track.
struct Opened {
    format: Box<dyn FormatReader>,
    track_id: u32,
    params: AudioCodecParameters,
    time_base: Option<TimeBase>,
    duration: Option<Duration>,
    file_len: u64,
}

/// Maps a Symphonia error to ours.
fn map_error(err: SymError) -> AudioError {
    match err {
        SymError::IoError(err) => AudioError::Io(err),
        SymError::Unsupported(what) => AudioError::Unsupported(what.to_string()),
        SymError::SeekError(kind) => AudioError::Seek(format!("{kind:?}")),
        other => AudioError::Decode(other.to_string()),
    }
}

/// Probes the container of `path` and finds its default audio track.
fn open_container(path: &Path) -> Result<Opened, AudioError> {
    let file = File::open(path)?;
    let file_len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(ext);
    }
    let format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|err| match err {
            SymError::Unsupported(_) => AudioError::Unsupported("unknown container".into()),
            other => map_error(other),
        })?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| AudioError::Unsupported("no audio track".into()))?;
    let Some(CodecParameters::Audio(params)) = &track.codec_params else {
        return Err(AudioError::Unsupported("no audio parameters".into()));
    };
    let params = params.clone();
    let (track_id, time_base, num_frames) = (track.id, track.time_base, track.num_frames);
    let duration = track_duration(num_frames, params.sample_rate, track.duration, time_base);
    Ok(Opened {
        format,
        track_id,
        params,
        time_base,
        duration,
        file_len,
    })
}

/// The playing time from the frame count when known, else from the container duration.
fn track_duration(
    num_frames: Option<u64>,
    sample_rate: Option<u32>,
    duration: Option<symphonia::core::units::Duration>,
    time_base: Option<TimeBase>,
) -> Option<Duration> {
    if let (Some(frames), Some(rate)) = (num_frames, sample_rate)
        && rate > 0
        && frames > 0
    {
        return Some(Duration::from_secs_f64(frames as f64 / f64::from(rate)));
    }
    let time = time_base?.calc_duration(duration?)?;
    let secs = time.as_secs_f64();
    (secs.is_finite() && secs > 0.0).then(|| Duration::from_secs_f64(secs))
}

/// Short codec name in capitals, as the badge shows it.
fn codec_name(id: AudioCodecId) -> String {
    use symphonia::core::codecs::audio::well_known::*;
    const PCM: &[AudioCodecId] = &[
        CODEC_ID_PCM_S32LE,
        CODEC_ID_PCM_S32LE_PLANAR,
        CODEC_ID_PCM_S32BE,
        CODEC_ID_PCM_S32BE_PLANAR,
        CODEC_ID_PCM_S24LE,
        CODEC_ID_PCM_S24LE_PLANAR,
        CODEC_ID_PCM_S24BE,
        CODEC_ID_PCM_S24BE_PLANAR,
        CODEC_ID_PCM_S16LE,
        CODEC_ID_PCM_S16LE_PLANAR,
        CODEC_ID_PCM_S16BE,
        CODEC_ID_PCM_S16BE_PLANAR,
        CODEC_ID_PCM_S8,
        CODEC_ID_PCM_S8_PLANAR,
        CODEC_ID_PCM_U32LE,
        CODEC_ID_PCM_U32LE_PLANAR,
        CODEC_ID_PCM_U32BE,
        CODEC_ID_PCM_U32BE_PLANAR,
        CODEC_ID_PCM_U24LE,
        CODEC_ID_PCM_U24LE_PLANAR,
        CODEC_ID_PCM_U24BE,
        CODEC_ID_PCM_U24BE_PLANAR,
        CODEC_ID_PCM_U16LE,
        CODEC_ID_PCM_U16LE_PLANAR,
        CODEC_ID_PCM_U16BE,
        CODEC_ID_PCM_U16BE_PLANAR,
        CODEC_ID_PCM_U8,
        CODEC_ID_PCM_U8_PLANAR,
        CODEC_ID_PCM_F32LE,
        CODEC_ID_PCM_F32LE_PLANAR,
        CODEC_ID_PCM_F32BE,
        CODEC_ID_PCM_F32BE_PLANAR,
        CODEC_ID_PCM_F64LE,
        CODEC_ID_PCM_F64LE_PLANAR,
        CODEC_ID_PCM_F64BE,
        CODEC_ID_PCM_F64BE_PLANAR,
        CODEC_ID_PCM_ALAW,
        CODEC_ID_PCM_MULAW,
    ];
    const ADPCM: &[AudioCodecId] = &[
        CODEC_ID_ADPCM_G722,
        CODEC_ID_ADPCM_G726,
        CODEC_ID_ADPCM_G726LE,
        CODEC_ID_ADPCM_MS,
        CODEC_ID_ADPCM_IMA_WAV,
        CODEC_ID_ADPCM_IMA_QT,
    ];
    if id == CODEC_ID_FLAC {
        "FLAC".into()
    } else if id == CODEC_ID_MP3 {
        "MP3".into()
    } else if id == CODEC_ID_VORBIS {
        "VORBIS".into()
    } else if id == CODEC_ID_AAC {
        "AAC".into()
    } else if id == CODEC_ID_ALAC {
        "ALAC".into()
    } else if id == CODEC_ID_OPUS {
        "OPUS".into()
    } else if PCM.contains(&id) {
        "PCM".into()
    } else if ADPCM.contains(&id) {
        "ADPCM".into()
    } else {
        symphonia::default::get_codecs()
            .get_audio_decoder(id)
            .map(|decoder| decoder.codec.info.short_name.to_uppercase())
            .unwrap_or_else(|| "UNKNOWN".into())
    }
}

/// Fills `meta` with the tags of `rev`, keeping what is already there.
fn collect_tags(rev: &MetadataRevision, meta: &mut TrackMeta, album_artist: &mut Option<String>) {
    let tags = rev
        .media
        .tags
        .iter()
        .chain(rev.per_track.iter().flat_map(|t| &t.metadata.tags));
    for tag in tags {
        let (slot, value) = match &tag.std {
            Some(StandardTag::TrackTitle(v)) => (&mut meta.title, v),
            Some(StandardTag::Artist(v)) => (&mut meta.artist, v),
            Some(StandardTag::Album(v)) => (&mut meta.album, v),
            Some(StandardTag::Genre(v)) => (&mut meta.genre, v),
            Some(StandardTag::AlbumArtist(v)) => (&mut *album_artist, v),
            _ => continue,
        };
        let value = repair_text(value.trim());
        if slot.is_none() && !value.is_empty() {
            *slot = Some(value);
        }
    }
}

impl Decoder for SymphoniaDecoder {
    fn open(&self, path: &Path) -> Result<Box<dyn DecodedStream>, AudioError> {
        let opened = open_container(path)?;
        let info = stream_info(&opened);
        let sample_rate = opened
            .params
            .sample_rate
            .ok_or_else(|| AudioError::Unsupported("unknown sample rate".into()))?;
        let channels = opened
            .params
            .channels
            .as_ref()
            .map(|channels| channels.count() as u16)
            .filter(|count| *count > 0)
            .ok_or_else(|| AudioError::Unsupported("unknown channel layout".into()))?;
        let options = AudioDecoderOptions::default().gapless(true);
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&opened.params, &options)
            .map_err(|err| match err {
                SymError::Unsupported(_) => {
                    AudioError::Unsupported(format!("no decoder for {}", info.codec))
                }
                other => map_error(other),
            })?;
        Ok(Box::new(SymphoniaStream {
            path: path.to_path_buf(),
            format: opened.format,
            decoder,
            track_id: opened.track_id,
            time_base: opened
                .time_base
                .or_else(|| TimeBase::try_from_recip(sample_rate)),
            sample_rate,
            channels,
            info,
            duration: opened.duration,
            skip_frames: 0,
            scratch: Vec::new(),
        }))
    }

    fn probe(&self, path: &Path) -> Result<Probe, AudioError> {
        let mut opened = open_container(path)?;
        let mut meta = TrackMeta {
            duration: opened.duration,
            ..TrackMeta::default()
        };
        let mut album_artist = None;
        let mut log = opened.format.metadata();
        loop {
            if let Some(rev) = log.current() {
                collect_tags(rev, &mut meta, &mut album_artist);
            }
            if log.is_latest() {
                break;
            }
            log.pop();
        }
        if meta.artist.is_none() {
            meta.artist = album_artist;
        }
        if meta.title.is_none() {
            // Symphonia drops the INFO tags of WAV files, so read them here.
            if let Some(tags) = read_wav_info(path) {
                meta.title = tags.title;
                meta.artist = meta.artist.or(tags.artist);
                meta.album = meta.album.or(tags.album);
                meta.genre = meta.genre.or(tags.genre);
            }
        }
        let info = stream_info(&opened);
        Ok(Probe { meta, info })
    }
}

/// Repairs UTF-8 text that was decoded as Latin-1 (some AIFF writers store UTF-8 in chunks
/// that are specified as Latin-1). Only when the Latin-1 chars form valid UTF-8 containing
/// at least one non-ASCII char is the UTF-8 reading used, so real Latin-1 text stays as is.
fn repair_text(text: &str) -> String {
    if text.is_ascii() || text.chars().any(|c| u32::from(c) > 0xFF) {
        return text.to_string();
    }
    let bytes: Vec<u8> = text.chars().map(|c| c as u8).collect();
    match String::from_utf8(bytes) {
        Ok(fixed) => fixed,
        Err(_) => text.to_string(),
    }
}

/// Most bytes of a LIST chunk that are read, and most chunks that are walked.
const MAX_LIST_BYTES: u64 = 64 * 1024;
const MAX_CHUNKS: usize = 256;

/// Text of an INFO sub-chunk: up to the first NUL, UTF-8 with a lossy fallback.
fn info_text(raw: &[u8]) -> Option<String> {
    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    let text = String::from_utf8_lossy(&raw[..end]);
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Tags found in a WAV `LIST`/`INFO` chunk.
#[derive(Default)]
struct WavInfo {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
}

/// Reads the `LIST`/`INFO` tags of a RIFF/WAVE file by walking its chunks. Sizes are never
/// trusted: reads are bounded, and the walk stops at the end of the file or after a limit.
fn read_wav_info(path: &Path) -> Option<WavInfo> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut header = [0u8; 12];
    file.read_exact(&mut header).ok()?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" {
        return None;
    }
    let mut pos = 12u64;
    let mut found = None;
    for _ in 0..MAX_CHUNKS {
        if pos + 8 > len {
            break;
        }
        file.seek(SeekFrom::Start(pos)).ok()?;
        let mut chunk = [0u8; 8];
        file.read_exact(&mut chunk).ok()?;
        let size = u64::from(u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]));
        let body = pos + 8;
        if &chunk[..4] == b"LIST" {
            let take = size.min(MAX_LIST_BYTES).min(len.saturating_sub(body));
            let mut data = vec![0u8; take as usize];
            file.read_exact(&mut data).ok()?;
            if let Some(info) = parse_info(&data) {
                found = Some(info);
                break;
            }
        }
        pos = body + size + (size & 1);
    }
    found
}

/// Parses the sub-chunks of a `LIST` body when it is an `INFO` list.
fn parse_info(data: &[u8]) -> Option<WavInfo> {
    if data.get(..4)? != b"INFO" {
        return None;
    }
    let mut info = WavInfo::default();
    let mut at = 4usize;
    while at + 8 <= data.len() {
        let id = &data[at..at + 4];
        let size = u32::from_le_bytes([data[at + 4], data[at + 5], data[at + 6], data[at + 7]]);
        let start = at + 8;
        let end = start.saturating_add(size as usize).min(data.len());
        let text = info_text(&data[start..end]);
        match id {
            b"INAM" => info.title = text,
            b"IART" => info.artist = text,
            b"IPRD" => info.album = text,
            b"IGNR" => info.genre = text,
            _ => {}
        }
        at = start
            .saturating_add(size as usize)
            .saturating_add(size as usize & 1);
    }
    Some(info)
}

/// Technical details of a probed track. The bitrate is the file average.
fn stream_info(opened: &Opened) -> StreamInfo {
    let bitrate_kbps = opened.duration.and_then(|duration| {
        let secs = duration.as_secs_f64();
        (opened.file_len > 0 && secs > 0.0)
            .then(|| (opened.file_len as f64 * 8.0 / secs / 1000.0).round() as u32)
    });
    StreamInfo {
        codec: codec_name(opened.params.codec),
        bitrate_kbps,
        sample_rate: opened.params.sample_rate.unwrap_or(0),
        channels: opened
            .params
            .channels
            .as_ref()
            .map(|c| c.count() as u16)
            .unwrap_or(0),
    }
}

/// A Symphonia stream being decoded.
struct SymphoniaStream {
    path: PathBuf,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    sample_rate: u32,
    channels: u16,
    info: StreamInfo,
    duration: Option<Duration>,
    /// Frames to drop from the output after a seek that landed before the target.
    skip_frames: usize,
    scratch: Vec<f32>,
}

impl SymphoniaStream {
    /// Seconds of a timestamp of this track.
    fn seconds(&self, ts: Timestamp) -> f64 {
        self.time_base
            .and_then(|base| base.calc_time(ts))
            .map(|time| time.as_secs_f64())
            .unwrap_or(0.0)
    }
}

impl DecodedStream for SymphoniaStream {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn info(&self) -> &StreamInfo {
        &self.info
    }

    fn duration(&self) -> Option<Duration> {
        self.duration
    }

    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, AudioError> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(false),
                // A chained stream wants new decoders: treated as the end of this track.
                Err(SymError::ResetRequired) => return Ok(false),
                Err(SymError::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(false);
                }
                Err(err) => return Err(map_error(err)),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buf) => {
                    let frames = buf.frames();
                    if frames == 0 {
                        continue;
                    }
                    buf.copy_to_vec_interleaved(&mut self.scratch);
                    let channels = usize::from(self.channels);
                    let skip = self.skip_frames.min(frames);
                    self.skip_frames -= skip;
                    if skip == frames {
                        continue;
                    }
                    let samples = &self.scratch[skip * channels..frames * channels];
                    out.extend_from_slice(samples);
                    return Ok(true);
                }
                Err(SymError::DecodeError(msg)) => log::debug!("skipping corrupt packet: {msg}"),
                Err(SymError::IoError(err)) => log::debug!("skipping unreadable packet: {err}"),
                Err(err) => return Err(map_error(err)),
            }
        }
    }

    fn seek(&mut self, position: Duration) -> Result<Duration, AudioError> {
        if position < REWIND_BELOW {
            // Symphonia's FLAC seek returns a broken stream when the target is inside the first
            // packet, so seeks near the start open the container again and skip to the target.
            self.format = open_container(&self.path)?.format;
            self.decoder.reset();
            self.skip_frames =
                (position.as_secs_f64() * f64::from(self.sample_rate)).round() as usize;
            return Ok(position);
        }
        let time = Time::try_from_secs_f64(position.as_secs_f64())
            .ok_or_else(|| AudioError::Seek("position out of range".into()))?;
        let to = SeekTo::Time {
            time,
            track_id: Some(self.track_id),
        };
        let seeked = self
            .format
            .seek(SeekMode::Accurate, to)
            .map_err(map_error)?;
        self.decoder.reset();
        let gap = self.seconds(seeked.required_ts) - self.seconds(seeked.actual_ts);
        let gap = gap.max(0.0);
        self.skip_frames = (gap * f64::from(self.sample_rate)).round() as usize;
        Ok(position)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::test_util::{fixture, sine_wav};

    /// Decodes a whole file, returning the interleaved samples.
    fn decode_all(stream: &mut dyn DecodedStream) -> Vec<f32> {
        let mut out = Vec::new();
        while stream.read(&mut out).unwrap() {}
        out
    }

    fn near(actual: usize, expected: usize, tolerance: usize) -> bool {
        actual.abs_diff(expected) <= tolerance
    }

    #[test]
    fn extensions_are_lowercase_and_matched_case_insensitively() {
        assert!(
            supported_extensions()
                .iter()
                .all(|e| e.to_ascii_lowercase() == *e)
        );
        assert!(is_supported_path(Path::new("/music/Song.MP3")));
        assert!(is_supported_path(Path::new("a.flac")));
        assert!(!is_supported_path(Path::new("cover.jpg")));
        assert!(!is_supported_path(Path::new("noextension")));
    }

    #[test]
    fn probe_reads_tags_duration_and_codec_of_each_fixture() {
        let cases = [
            ("sine.flac", "FLAC", 1),
            ("sine.mp3", "MP3", 2),
            ("sine.ogg", "VORBIS", 2),
        ];
        for (name, codec, channels) in cases {
            let probe = SymphoniaDecoder.probe(&fixture(name)).unwrap();
            assert_eq!(probe.meta.title.as_deref(), Some("Sine Test"), "{name}");
            assert_eq!(probe.meta.artist.as_deref(), Some("Nastro Tests"), "{name}");
            assert_eq!(probe.meta.album.as_deref(), Some("Fixtures"), "{name}");
            assert_eq!(probe.meta.genre.as_deref(), Some("Test Tone"), "{name}");
            let duration = probe.meta.duration.expect(name);
            assert!(
                duration.abs_diff(Duration::from_secs(1)) < Duration::from_millis(60),
                "{name}"
            );
            assert_eq!(probe.info.codec, codec, "{name}");
            assert_eq!(probe.info.sample_rate, 44_100, "{name}");
            assert_eq!(probe.info.channels, channels, "{name}");
            assert!(
                probe.info.bitrate_kbps.is_some_and(|kbps| kbps > 10),
                "{name}"
            );
        }
    }

    #[test]
    fn decoding_each_fixture_gives_about_one_second() {
        for name in ["sine.flac", "sine.mp3", "sine.ogg"] {
            let mut stream = SymphoniaDecoder.open(&fixture(name)).unwrap();
            let channels = usize::from(stream.channels());
            let samples = decode_all(stream.as_mut());
            assert!(
                near(samples.len() / channels, 44_100, 2_400),
                "{name}: {}",
                samples.len()
            );
            let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak > 0.05 && peak <= 1.0, "{name}: peak {peak}");
            assert!(
                !stream.read(&mut Vec::new()).unwrap(),
                "{name}: still ended"
            );
        }
    }

    #[test]
    fn seek_lands_on_the_target() {
        for name in ["sine.flac", "sine.mp3", "sine.ogg"] {
            let mut stream = SymphoniaDecoder.open(&fixture(name)).unwrap();
            let channels = usize::from(stream.channels());
            let reached = stream.seek(Duration::from_millis(500)).unwrap();
            assert_eq!(reached, Duration::from_millis(500));
            let rest = decode_all(stream.as_mut()).len() / channels;
            assert!(near(rest, 22_050, 2_400), "{name}: {rest} frames left");
            // And back to the start.
            stream.seek(Duration::ZERO).unwrap();
            let all = decode_all(stream.as_mut()).len() / channels;
            assert!(near(all, 44_100, 2_400), "{name}: {all} frames");
        }
    }

    #[test]
    fn seeks_near_the_start_are_exact_for_every_format() {
        for name in ["sine.flac", "sine.mp3", "sine.ogg"] {
            let mut stream = SymphoniaDecoder.open(&fixture(name)).unwrap();
            let channels = usize::from(stream.channels());
            let whole = decode_all(stream.as_mut()).len() / channels;
            for ms in [0u64, 3, 9, 15, 21, 100] {
                assert_eq!(
                    stream.seek(Duration::from_millis(ms)).unwrap(),
                    Duration::from_millis(ms)
                );
                let rest = decode_all(stream.as_mut()).len() / channels;
                let skipped = (ms as f64 * 44.1).round() as usize;
                assert_eq!(rest, whole - skipped, "{name} at {ms} ms");
            }
        }
    }

    #[test]
    fn wav_decodes_exactly_and_seeks_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let path = sine_wav(dir.path(), "a.wav", 48_000, 1, 1.0);
        let probe = SymphoniaDecoder.probe(&path).unwrap();
        assert_eq!(probe.info.codec, "PCM");
        assert_eq!(probe.meta.duration, Some(Duration::from_secs(1)));
        let mut stream = SymphoniaDecoder.open(&path).unwrap();
        assert_eq!((stream.sample_rate(), stream.channels()), (48_000, 1));
        assert_eq!(stream.duration(), Some(Duration::from_secs(1)));
        assert_eq!(decode_all(stream.as_mut()).len(), 48_000);
        stream.seek(Duration::from_millis(250)).unwrap();
        assert_eq!(decode_all(stream.as_mut()).len(), 36_000);
    }

    #[test]
    fn errors_are_typed() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.mp3");
        assert!(matches!(
            SymphoniaDecoder.open(&missing),
            Err(AudioError::Io(_))
        ));
        assert!(matches!(
            SymphoniaDecoder.probe(&missing),
            Err(AudioError::Io(_))
        ));
        let junk = dir.path().join("junk.mp3");
        std::fs::write(&junk, vec![7u8; 4096]).unwrap();
        assert!(matches!(
            SymphoniaDecoder.probe(&junk),
            Err(AudioError::Unsupported(_))
        ));
    }

    #[test]
    fn a_truncated_file_decodes_what_is_there() {
        let bytes = std::fs::read(fixture("sine.mp3")).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cut.mp3");
        std::fs::write(&path, &bytes[..bytes.len() * 2 / 3]).unwrap();
        let mut stream = SymphoniaDecoder.open(&path).unwrap();
        let frames = decode_all(stream.as_mut()).len() / 2;
        assert!(frames > 10_000 && frames < 44_100, "{frames}");
    }

    /// A WAV with a `LIST`/`INFO` chunk before the data.
    fn wav_with_info(path: &Path, tags: &[(&[u8; 4], &str)]) {
        let mut list = b"INFO".to_vec();
        for (id, text) in tags {
            let mut raw = text.as_bytes().to_vec();
            raw.push(0);
            list.extend_from_slice(*id);
            list.extend_from_slice(&(raw.len() as u32).to_le_bytes());
            list.extend_from_slice(&raw);
            if raw.len() % 2 == 1 {
                list.push(0);
            }
        }
        let pcm = vec![0u8; 8000];
        let mut bytes = b"RIFF".to_vec();
        let total = 4 + 24 + 8 + list.len() + 8 + pcm.len();
        bytes.extend_from_slice(&(total as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        for v in [1u16, 1] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&8000u32.to_le_bytes());
        bytes.extend_from_slice(&16000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"LIST");
        bytes.extend_from_slice(&(list.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&list);
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&pcm);
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn wav_info_tags_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.wav");
        wav_with_info(
            &path,
            &[
                (b"INAM", "Valse Électrique"),
                (b"IART", "Ana"),
                (b"IPRD", "Odd"),
                (b"IGNR", "Pop"),
            ],
        );
        let meta = SymphoniaDecoder.probe(&path).unwrap().meta;
        assert_eq!(meta.title.as_deref(), Some("Valse Électrique"));
        assert_eq!(meta.artist.as_deref(), Some("Ana"));
        assert_eq!(meta.album.as_deref(), Some("Odd"));
        assert_eq!(meta.genre.as_deref(), Some("Pop"));
    }

    #[test]
    fn wav_without_info_has_no_tags_and_a_bad_list_is_harmless() {
        let dir = tempfile::tempdir().unwrap();
        let plain = sine_wav(dir.path(), "p.wav", 8_000, 1, 0.1);
        assert_eq!(SymphoniaDecoder.probe(&plain).unwrap().meta.title, None);
        // A LIST chunk claiming a huge size must not break the walk.
        let mut bytes = std::fs::read(&plain).unwrap();
        bytes.extend_from_slice(b"LIST\xff\xff\xff\x7fINFOINAM");
        let bad = dir.path().join("bad.wav");
        std::fs::write(&bad, bytes).unwrap();
        assert!(SymphoniaDecoder.probe(&bad).is_ok());
    }

    #[test]
    fn real_samples_keep_their_tags_when_present() {
        let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/audio");
        for (file, title) in [
            ("04-hat-and-noise.wav", "Hat and Noise"),
            ("08-valse-électrique.wav", "Valse Électrique"),
            ("06-silencio-en-el-café.aiff", "Silencio en el Café"),
        ] {
            let path = samples.join(file);
            if path.exists() {
                let meta = SymphoniaDecoder.probe(&path).unwrap().meta;
                assert_eq!(meta.title.as_deref(), Some(title), "{file}");
            }
        }
    }

    #[test]
    fn repair_text_fixes_only_mojibake() {
        assert_eq!(repair_text("Silencio en el CafÃ©"), "Silencio en el Café");
        assert_eq!(repair_text("Café"), "Café");
        assert_eq!(repair_text("Plain"), "Plain");
        assert_eq!(repair_text("日本"), "日本");
        assert_eq!(repair_text("Ã"), "Ã");
    }
}
