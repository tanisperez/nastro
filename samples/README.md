# Samples

`generate.sh` makes a small set of synthetic tracks for manual checks of the player, the
visualizer and the equalizer. Run it with `make samples` (or `samples/generate.sh`). It needs
only `ffmpeg` and `ffprobe` (with `libmp3lame`, `libvorbis`, `flac` and `aac`) and writes into
`samples/audio/`, which git ignores. Never commit real music or the generated files.

The tracks are made of sums of sines with envelopes (chords, arpeggios, a kick, a sweep), plus
filtered noise for the hi-hat. They are deterministic: the same ffmpeg gives the same files. The
run takes a few minutes, most of it the 4 minute track.

| File | What it is | Format | Rate |
|---|---|---|---|
| `01-aurora-chords.flac` | Chord pad, Am F C G, 45 s | FLAC 16-bit | 44.1 kHz |
| `02-glass-arpeggio.mp3` | Fast arpeggio, 30 s | MP3 320 kbps | 44.1 kHz |
| `03-low-thump.ogg` | Kick and bass pattern, 30 s, for the low bands | Ogg Vorbis | 48 kHz |
| `04-hat-and-noise.wav` | Filtered noise hi-hat, 30 s, for the high bands | WAV 16-bit | 44.1 kHz |
| `05-sweep-20hz-20khz.flac` | Logarithmic sweep, 20 s | FLAC 24-bit | 96 kHz |
| `06-silencio-en-el-café.aiff` | Very quiet passage, 45 s (accents in the title and the file name) | AIFF 16-bit | 44.1 kHz |
| `07-long-evening.mp3` | The long track, 4 minutes | MP3 VBR (`-q:a 2`) | 44.1 kHz |
| `08-valse-électrique.wav` | Waltz, 36 s (accents in the title and the file name) | WAV 24-bit | 48 kHz |
| `09-pulse-and-pad.m4a` | Kick, pad and arpeggio together, 40 s | M4A (AAC 192 kbps) | 48 kHz |
| `10-radio-de-onda-corta.mp3` | Small tune, 24 s, mono | MP3 64 kbps mono | 22.05 kHz |
| `11-ping-pong.ogg` | Notes bouncing between left and right, 32 s, for balance | Ogg Vorbis | 44.1 kHz |
| `12-band-ladder.flac` | One tone per equalizer band, 31 Hz to 16 kHz, 2 s each, then all together | FLAC 16-bit | 44.1 kHz |

Every file has a title, artist, album artist, album, genre and track number (`n/12`). The artists
are invented (Lumen Harbor, Subterra Club, Test Signal Orchestra, Marrow Field, Zoë Quartz,
Orquesta Sintética del Sur) and some names have accents, to test text filtering.

`audio/sample.m3u8` lists the tracks in order with `#EXTINF` (duration in seconds, `Artist -
Title`) and relative paths, so it also tests playlist loading.

Handy checks:

- Equalizer: play `12-band-ladder.flac` and move one band at a time; the tone of that band
  changes level.
- Visualizer: `05-sweep-20hz-20khz.flac` walks across every bar, `03-low-thump.ogg` fills the
  left side and `04-hat-and-noise.wav` the right side.
- Gapless playback and seeking: `07-long-evening.mp3` for seeking inside a long VBR file.
- Resampling: the mix of 22.05, 44.1, 48 and 96 kHz files in the playlist.
