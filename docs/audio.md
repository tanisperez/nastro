# Audio

How `nastro-audio` works, for people changing it. The crate depends only on `nastro-core`; the
app talks to it through `Player` and never sees a Symphonia, cpal or rubato type.

## Pipeline

```text
file -> decode -> equalizer -> resample -> channel layout -> ring -> output stage -> device
        (engine thread, per chunk)                           (device callback)    |
                                                                                  +-> analysis tap
```

1. **Decode.** A `Decoder` opens a file as a `DecodedStream` that yields interleaved `f32`
   chunks in `[-1, 1]`. Isolated corrupt packets are skipped.
2. **Equalizer.** Ten RBJ peaking biquads (Q 1.41, one per `EQ_BANDS_HZ`) plus the preamp as a
   linear gain, run in `f64` at the source sample rate. A disabled or flat equalizer is a true
   bypass (bit identical). Bands at or above 0.45 of the sample rate are skipped. Changing the
   settings recomputes coefficients but keeps the filter state, so sliders do not click.
3. **Resample.** Only when the source rate differs from the device rate: a rubato `Async` sinc
   resampler, `sinc_len` 128, `BlackmanHarris2` window, linear interpolation of the sinc table,
   oversampling 256, fixed input chunks of 1024 frames. The start delay is trimmed and the tail
   flushed at the end of a stream.
4. **Channel layout.** Converted to the device layout: mono is copied to both sides, more than
   two channels are downmixed to stereo, extra device channels get silence.
5. **Ring.** A lock free `rtrb` ring holding 0.3 s of device frames between the engine thread
   and the device callback. That 0.3 s is the engine's latency for everything that is queued
   ahead (seeks, stops and track changes flush it, so they are immediate).
6. **Output stage** (`Renderer`, run by the device callback). Drains the ring, applies volume
   and balance with a 10 ms ramp, copies a mono mix of the first two channels into a 4096
   sample tap, counts the frames it consumed, and writes silence on an underrun. It never
   allocates, blocks, logs or waits for a lock (`try_lock` and skip on contention).
7. **Analysis.** The UI calls `Player::analysis` about 30 times a second. It reads the tap
   (taken before the volume gain, so the bars do not vanish at low volume) and runs the
   `Analyzer`.

Volume and balance are applied in the output stage from atomics, so they take effect without
touching the engine thread.

### Volume and balance

- Volume: the slider fraction in `0..=1` maps to an amplitude of `fraction^3`. Half way is
  -18 dB, a tenth is -60 dB, zero is true silence.
- Balance in `-1..=1`: linear pan. The side away from the balance is attenuated in proportion
  (`left = min(1, 1 - b)`, `right = min(1, 1 + b)`); the centre leaves both at unity, so
  there is no level change at the default.
- Gains move per sample towards their target, going from silence to full scale in 10 ms.

### Analyzer

- FFT of the latest 2048 mono samples (realfft) with a Hann window. The plan and all buffers
  are created once; there is no allocation per frame.
- 24 bars, log spaced from 40 Hz to 16 kHz; each bar is the loudest bin in its range.
- Magnitude is normalized so a full scale sine is 0 dB, mapped to `0..1` over 60 dB, with a
  tilt of +3 dB per octave above 1 kHz to compensate the fall of music.
- Smoothing per frame: attack 0.75, decay 0.18 (fractions of the distance to the target).
  Peaks hold 8 frames, then fall with a gravity of 0.004 per frame, accelerating.
- The scope is 64 points, one every 16 samples over the latest 1024.
- When no new samples arrived (paused, stopped, underrun) silence is fed so the bars fall.
- `AnalysisFrame` is `bars`, `peaks` (24 values each) and `scope` (64 values).

## Threads and idle behavior

- One **engine thread** (`nastro-audio`) owns the decoder stream, the pipeline and the cpal
  stream (cpal streams may be `!Send`, so the sink is built on that thread through a factory).
- The **device callback** runs on the host's audio thread.
- `Player` is a cheap `Clone + Send + Sync` handle: methods send a command over an `mpsc`
  channel and return. Results come back as events on an unbounded `async_channel`, so the UI
  awaits it and needs no polling. Dropping the last handle shuts the thread down and joins it.
- **Idle rule.** While playing, the thread decodes one chunk at a time and, when the ring is
  full, sleeps at most 10 ms. When paused or stopped it blocks on the command channel with no
  timeout, and the cpal stream is paused, so neither the thread nor the device does any work.
  Playback ends the same way: after `Finished` the stream is paused and the thread blocks.
- The device is opened lazily on the first play. `Player::new` never fails; a missing device
  shows up as a `PlayerEvent::Error`.

## Public API in short

- `Decoder { open, probe }`, `DecodedStream { sample_rate, channels, info, duration, read,
  seek }`, `Probe { meta: TrackMeta, info: StreamInfo }`, `SymphoniaDecoder`,
  `supported_extensions()` and `is_supported_path()` (used to filter dropped files).
- `probe` is cheap: container metadata and codec parameters only, never a decode. It returns
  title, artist (album artist as fallback), album, genre, duration, codec name in capitals
  (`FLAC`, `MP3`, `VORBIS`, `AAC`, `ALAC`, `PCM`, `ADPCM`, `OPUS`) and the average bitrate from
  file size and duration.
- `Player`: `load(path, autoplay)`, `queue_next(Option<path>)`, `play`, `pause`, `stop`,
  `seek`, `set_volume`, `set_balance`, `set_eq`, `status()`, `analysis(&mut AnalysisFrame)`,
  `events()`. `Player::with_parts(make_sink, decoder)` is hidden and meant for tests.
- `Equalizer` and `response_db(settings, hz, rate)` (total response including preamp, usable
  for an accurate curve in the UI).
- `AudioSink` and `Renderer`: the output abstraction. `CpalSink` is the real one; tests use an
  in-memory sink in `engine/test_sink.rs` that drains the renderer faster than real time.

### Events

| Event | Meaning |
| --- | --- |
| `Started { path, info, duration }` | A track became the current one. Sent after every successful `load`, **including `load(path, false)`** (the display needs the info and duration; it does not mean audio is running), and after a gapless move, right after `Advanced`. |
| `Advanced { from, to }` | Playback crossed, without a gap, into the track given to `queue_next`. Sent when the audio of `to` is actually reached, not when it was decoded. |
| `Finished { path }` | The track reached its end with nothing queued and the output drained. The player is now stopped (a `StateChanged(Stopped)` comes first). |
| `StateChanged(state)` | The transport state changed. |
| `Error { path, message }` | A file could not be opened or decoded, a seek failed, or the device failed. After a failed `load` nothing is loaded. |

### Status

`status()` locks a small timeline briefly and reads one atomic, so it is safe to call every
frame. It returns the state, the current path, position, duration and stream info.

## Gapless playback and queue_next

`queue_next(Some(path))` names the successor. When the current stream ends the engine opens
it and keeps writing into the ring without draining it:

- **Same format fast path.** If the successor has the same sample rate and channel count, the
  pipeline (equalizer state and resampler) is kept, nothing is flushed, and the output is
  continuous. Gapless trimming (encoder delay and padding) is done by Symphonia for MP3,
  FLAC, Vorbis and ALAC.
- **Reformat case.** If the rate or channel count differ, the tail of the old pipeline is
  flushed into the ring and a new pipeline is built. The data is still contiguous in the
  ring, so in practice there is no audible gap; a tiny one would be acceptable.
- `queue_next(None)` clears the successor. `load` also forgets it, so the app calls
  `queue_next` again after each `Started`.
- If the queued file fails to open, an `Error` is sent and the player finishes normally.
- The boundary is placed at the frames produced so far, so with resampling it can be off by
  the resampler delay (about 1.5 ms).

## Position accounting

The device callback counts the frames it consumed (`played`). The engine records, for each
track in the ring, a segment: the absolute output frame where it starts and the position in
the track of its first frame. The position is `start_pos + (played - start_abs) / device_rate`
for the segment that contains `played`, clamped to the duration, so it follows what leaves
the ring and moves to the next track exactly when its audio is reached.

Flushing (seek, stop, load) drains the ring from the engine thread while holding the consumer
lock, adds the drained frames to `played`, and starts a new segment, so `produced`, pushed and
played are the same number afterwards. That is what makes the position exact right after a
seek. While stopped the position is the segment's start (0 after `stop`).

If the stream has already moved on to the queued track when the user seeks the heard one, the
engine reopens the heard track and puts the other back as the queued track.

## Seeking and Symphonia workarounds

`seek` uses accurate mode and skips the frames between the packet reached and the target, so
`DecodedStream::seek` returns the requested position. Things found while testing with real
files, all handled in the crate:

- **FLAC near the start.** Symphonia 0.6.1 returns a broken stream when the target falls in
  the first packet (about 23 ms), and seeking to 0 after the end of the stream does the same.
  Any seek below 200 ms therefore reopens the container and skips frames to the target.
- **Ogg near the end.** A seek within about 1 ms of the end returns `OutOfRange`. The engine
  clamps seeks to `duration - 1 ms`; a failure is reported as an `Error` event and playback
  carries on.
- **WAV INFO tags.** `symphonia-format-riff` reads the `LIST`/`INFO` chunk into a local log
  and then builds the reader with other metadata, so the tags are lost. When the probed
  metadata has no title, `probe` walks the RIFF chunks itself (bounded reads, sizes never
  trusted, at most 256 chunks and 64 KB per LIST) and reads `INAM` (title), `IART` (artist),
  `IPRD` (album) and `IGNR` (genre) as NUL terminated text, UTF-8 with a lossy fallback.
- **UTF-8 in AIFF.** `NAME` and `AUTH` text written as UTF-8 is decoded as Latin-1 and comes
  out as mojibake. Every tag value goes through a repair: if all chars are at most U+00FF and,
  taken as bytes, form valid UTF-8 with at least one non ASCII char, the UTF-8 reading is used.
  Real Latin-1 text and ASCII are left alone.
- **ALSA errno 77 on pause.** cpal on ALSA refuses to pause a stream that never ran
  (`snd_pcm_pause` fails with errno 77). The sink tries to pause right after building the
  stream and treats a refusal as harmless; later pause and resume work normally.
- A chained Ogg stream (`ResetRequired`) is treated as the end of the track.

## Versions and decisions

| Piece | Crate |
| --- | --- |
| Decoding | `symphonia` 0.6.1, feature `all` |
| Output | `cpal` 0.18.2 |
| Resampling | `rubato` 5.0.1 (re-exports `audioadapter`, used for its buffers) |
| Analysis | `realfft` 3.5.0 |
| Ring | `rtrb` 0.4 |
| Events | `async-channel` 2.5 |

All of them have APIs that differ a lot from older versions; read the sources under
`~/.cargo/registry/src/*/` rather than trusting memory.

**Why not `rodio`.** It hides the decode, resample and output chain. Nastro needs the
equalizer between decode and resample, the exact samples being heard for the visualizer, a
frame accurate position and gapless transitions, and `rodio` offers none of those hooks.

**Rejected: BASS (un4seen).** Closed source; free only for products that make no money;
commercial licences are per platform (950 EUR single commercial, 125 EUR shareware, 40 % off
each extra platform). See `design/handoff.md`, section 7.

## Not done yet

- Opus: probed and shown as `OPUS`, but there is no decoder (Symphonia has none yet; the
  plan was the `opus` crate over native libopus). `.ogg` files with Opus fail to open.
- WavPack and APE (Monkey's Audio): not decodable; APE is only a tag format in Symphonia.
- AAC gapless: Symphonia's AAC decoder is without gapless support.
- A `ffmpeg-next` fallback behind a cargo feature, off by default, for exotic formats.
- A fade on pause and stop: the stream is cut abruptly, which can click.
- ReplayGain.
- Surround downmix uses simple fixed weights assuming the usual channel order.

## Tests and examples

```sh
CARGO_TARGET_DIR=target/audio cargo test -p nastro-audio
CARGO_TARGET_DIR=target/audio cargo clippy -p nastro-audio --all-targets -- -D warnings
cargo fmt -p nastro-audio
```

The tests open no device and need no network; they run in well under a second. Fixtures are
generated inside the tests (a tiny WAV writer in `src/test_util.rs`) or committed under
`crates/nastro-audio/tests/fixtures/` (FLAC, MP3 and Ogg, with the exact ffmpeg commands in
its `README.md`). One test reads the tags of the files in `samples/audio/` when they exist
(`make samples`).

To check the real device path by ear, with a timeout:

```sh
cargo run -p nastro-audio --example play -- some-file.wav 8
cargo run -p nastro-audio --example play -- some-file.wav 8 pause   # also pauses for a second
```

`examples/play.rs` plays the file with `Player::new()`, prints the status once a second and
every event, and exits on `Finished` or `Error`.
