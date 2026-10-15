# Audio fixtures

Tiny files for the decoder tests: one second of a 440 Hz sine at 44.1 kHz (48 kHz for WebM),
with the tags `title=Sine Test`, `artist=Nastro Tests`, `album=Fixtures`, `genre=Test Tone`.
They were made with ffmpeg 7 (the Opus ones with ffmpeg 9) and these exact commands, run
inside this directory.

```sh
# FLAC, mono, 16 bit
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=44100" -ac 1 -sample_fmt s16 \
  -c:a flac -compression_level 8 -metadata title="Sine Test" -metadata artist="Nastro Tests" \
  -metadata album="Fixtures" -metadata genre="Test Tone" sine.flac

# MP3, stereo, 64 kbit/s, ID3v2.3 tags
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=44100" -ac 2 \
  -c:a libmp3lame -b:a 64k -id3v2_version 3 -metadata title="Sine Test" \
  -metadata artist="Nastro Tests" -metadata album="Fixtures" -metadata genre="Test Tone" sine.mp3

# Ogg Vorbis, stereo, quality 1, Vorbis comments
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=44100" -ac 2 \
  -c:a libvorbis -q:a 1 -metadata title="Sine Test" -metadata artist="Nastro Tests" \
  -metadata album="Fixtures" -metadata genre="Test Tone" sine.ogg

# Ogg Opus, stereo, 48 kbit/s, Vorbis comments
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=44100" -ac 2 \
  -c:a libopus -b:a 48k -metadata title="Sine Test" -metadata artist="Nastro Tests" \
  -metadata album="Fixtures" -metadata genre="Test Tone" sine.opus

# WebM Opus, mono, 32 kbit/s, Matroska tags
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=48000" -ac 1 \
  -c:a libopus -b:a 32k -metadata title="Sine Test" -metadata artist="Nastro Tests" \
  -metadata album="Fixtures" -metadata genre="Test Tone" sine.webm
```
