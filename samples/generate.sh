#!/usr/bin/env bash
# Generates a small set of synthetic tracks into samples/audio/ with ffmpeg only, for manual
# checks of a music player: formats, sample rates, tags, the visualizer and the equalizer.
# Deterministic: the same ffmpeg gives the same files. Never commit the output (it is ignored).
#
#   samples/generate.sh            # or: make samples
#
# Needs ffmpeg and ffprobe with libmp3lame, libvorbis, flac and aac.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
for tool in ffmpeg ffprobe awk; do
    command -v "$tool" > /dev/null || { echo "error: $tool not found on PATH" >&2; exit 1; }
done

out=audio
mkdir -p "$out"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# ---------------------------------------------------------------------------------------------
# Expression helpers. Every emitter adds a segment (a note) to a voice. A voice plays one note
# at a time, and compose() turns each voice into a balanced tree of if(lt(...)) tests, so only
# the sounding note is evaluated per sample (a flat sum of all notes is far too slow, and too
# deep for ffmpeg's parser). $TT is the time expression of the pattern (t, or mod(t,period) for
# a loop) and $DET detunes every pitch a little, so the right channel differs from the left.
# ---------------------------------------------------------------------------------------------
declare -A SEGS SEGTT
TT=t DET=1 SCALE=1 PAR=0 E=0

hz() { awk -v n="$1" -v d="$DET" 'BEGIN { printf "%.3f", 440 * 2 ^ ((n - 69) / 12) * d }'; }
num() { awk "BEGIN { printf \"%.4f\", $1 }"; }

# seg KEY START END TERM: a note of voice KEY, in time order, never overlapping the previous one.
seg() {
    SEGS[$1]+="$2|$3|$4"$'\n'
    SEGTT[$1]=$TT
}

# pad KEY START DUR AMP MIDI: slow swell, a sine and a soft second harmonic.
pad() {
    local f
    f=$(hz "$5")
    seg "$1" "$2" "$(num "$2+$3")" \
        "$4*sin(PI*($TT-$2)/$3)*sin(PI*($TT-$2)/$3)*(sin(2*PI*$f*t)+0.3*sin(4*PI*$f*t))"
}

# pluck KEY START LEN AMP DECAY MIDI: fast attack, exponential decay, three harmonics.
pluck() {
    local f
    f=$(hz "$6")
    seg "$1" "$2" "$(num "$2+$3")" \
        "$4*(1-exp(-($TT-$2)*400))*exp(-($TT-$2)*$5)*(1-($TT-$2)/$3)*(sin(2*PI*$f*t)+0.4*sin(4*PI*$f*t)+0.15*sin(6*PI*$f*t))"
}

# kick KEY START AMP: a sine that falls from 150 Hz to 50 Hz in a few milliseconds.
kick() {
    seg "$1" "$2" "$(num "$2+0.45")" \
        "$3*exp(-9*($TT-$2))*sin(2*PI*(50*($TT-$2)+2.5*(1-exp(-40*($TT-$2)))))"
}

# tone KEY START DUR AMP HZ: a plain sine with a smooth envelope, in exact hertz (no detune).
tone() {
    seg "$1" "$2" "$(num "$2+$3")" \
        "$4*sin(PI*(t-$2)/$3)*sin(PI*(t-$2)/$3)*sin(2*PI*$5*t)"
}

# tree LO HI: a balanced if() tree over the segments LO..HI-1 of the voice loaded in V_*.
tree() {
    local lo=$1 hi=$2 mid
    if ((hi - lo == 1)); then
        printf 'if(between(%s,%s,%s),%s,0)' "$VTT" "${V_START[lo]}" "${V_END[lo]}" "${V_TERM[lo]}"
    else
        mid=$(((lo + hi) / 2))
        printf 'if(lt(%s,%s),%s,%s)' "$VTT" "${V_START[mid]}" "$(tree "$lo" "$mid")" \
            "$(tree "$mid" "$hi")"
    fi
}

# compose: E becomes the sum of the voices, and the segments are cleared.
compose() {
    local key line
    E=0
    for key in "${!SEGS[@]}"; do
        V_START=() V_END=() V_TERM=()
        while IFS='|' read -r s e term; do
            [[ -n $s ]] || continue
            V_START+=("$s") V_END+=("$e") V_TERM+=("$term")
        done <<< "${SEGS[$key]}"
        VTT=${SEGTT[$key]}
        E+="+$(tree 0 "${#V_START[@]}")"
    done
    SEGS=() SEGTT=()
}

# The chord progression shared by several tracks: Am, F, C, G, four seconds each (16 s loop).
CH0=(57 60 64) CH1=(53 57 60) CH2=(55 60 64) CH3=(55 59 62)
ROOTS=(33 29 36 31)

pad_prog() { # AMP
    local i j
    TT="mod(t,16)"
    for i in 0 1 2 3; do
        local -n chord="CH$i"
        for j in 0 1 2; do pad "pad$j" $((i * 4)) 4 "$1" "${chord[$j]}"; done
        pad padb $((i * 4)) 4 "$(num "$1*1.4")" "${ROOTS[$i]}"
    done
}

arp_prog() { # STEP AMP: an up and down arpeggio over each chord, two voices taking turns
    local step=$1 a=$2 i k idx
    TT="mod(t,16)"
    local seq=(0 1 2 3 2 1)
    local count
    count=$(awk "BEGIN { print int(4 / $step) }")
    for i in 0 1 2 3; do
        local -n chord="CH$i"
        local notes=("${chord[@]}" $((chord[0] + 12)))
        for ((k = 0; k < count; k++)); do
            idx=${seq[$((k % 6))]}
            pluck "arp$((k % 2))" "$(num "$i*4+$k*$step")" "$(num "$step*2")" "$a" 7 $((notes[idx] + 12))
        done
    done
}

kick_pat() { # AMP: four on the floor at 120 bpm plus an off-beat bass note
    local b
    TT="mod(t,2)"
    for b in 0 0.5 1 1.5; do kick kick "$b" "$1"; done
    for b in 0.25 0.75 1.25 1.75; do pluck kbass "$b" 0.4 0.3 5 33; done
}

waltz() { # a 12 s loop of eight bars in 3/4: bass note, then two chords
    local roots=(45 50 52 45 41 50 52 45)
    local chords=("57 60 64" "62 65 69" "59 64 68" "57 60 64" "57 60 65" "62 65 69" "59 64 68" "57 60 64")
    local i j b notes
    TT="mod(t,12)"
    for i in 0 1 2 3 4 5 6 7; do
        pluck wr "$(num "$i*1.5")" 0.7 0.25 4 "${roots[$i]}"
        read -ra notes <<< "${chords[$i]}"
        for b in 0 1; do
            for j in 0 1 2; do
                pluck "wc${b}_$j" "$(num "$i*1.5+0.5+$b*0.5")" 0.5 0.09 3 "${notes[$j]}"
            done
        done
    done
}

melody() { # a four second tune, eight notes
    local seq=(67 69 71 72 71 69 67 64) i
    TT="mod(t,4)"
    for i in 0 1 2 3 4 5 6 7; do pluck mel "$(num "$i*0.5")" 0.5 0.2 3 "${seq[$i]}"; done
}

pingpong() { # notes that alternate between the channels: PAR picks the even or odd steps
    local penta=(60 62 64 67 69 72) i
    TT="mod(t,6.4)"
    for ((i = 0; i < 32; i++)); do
        ((i % 2 == PAR)) || continue
        pluck ping "$(num "$i*0.2")" 0.4 0.3 5 "${penta[$(((i * 5 + i / 3) % 6))]}"
    done
}

# Builds EL and ER (the left and right expressions) by running an emitter twice.
build() {
    DET=1
    "$@"
    compose
    EL=$E
    DET=1.004
    "$@"
    compose
    ER=$E
    DET=1
}

# Escapes the commas of an expression for the filter graph.
esc() { printf '%s' "${1//,/\\,}"; }

# stereo_src DUR RATE: the aevalsrc source for EL and ER, clipped for safety.
stereo_src() {
    printf 'aevalsrc=exprs=%s|%s:s=%s:d=%s:c=stereo' \
        "$(esc "clip($SCALE*($EL),-0.95,0.95)")" "$(esc "clip($SCALE*($ER),-0.95,0.95)")" "$2" "$1"
}

# ---------------------------------------------------------------------------------------------
# Encoding and the playlist.
# ---------------------------------------------------------------------------------------------
TRACKS=()
N=0

# encode FILE DURATION SOURCE "ENCODER ARGS" TITLE ARTIST ALBUM GENRE
# SOURCE is a filter graph that makes the audio; a short fade in and out is added to it.
encode() {
    local file=$1 dur=$2 src=$3 enc=$4 title=$5 artist=$6 album=$7 genre=$8
    local fade_out
    fade_out=$(num "$dur-0.8")
    N=$((N + 1))
    printf '%s,afade=t=in:d=0.05,afade=t=out:st=%s:d=0.8[a]' "$src" "$fade_out" > "$work/graph.txt"
    # shellcheck disable=SC2086
    ffmpeg -nostdin -v error -y -/filter_complex "$work/graph.txt" -map '[a]' \
        -map_metadata -1 -fflags +bitexact -flags:a +bitexact $enc \
        -metadata title="$title" -metadata artist="$artist" -metadata album_artist="$artist" \
        -metadata album="$album" -metadata genre="$genre" -metadata track="$N/12" \
        "$out/$file"
    TRACKS+=("$file|$artist|$title")
    echo "  $file"
}

echo "Generating tracks into samples/$out/"

# 1. Chord pad, FLAC 44.1 kHz.
SCALE=1
build pad_prog 0.1
encode 01-aurora-chords.flac 45 "$(stereo_src 45 44100)" "-c:a flac -sample_fmt s16" \
    "Aurora Chords" "Lumen Harbor" "Synthetic Seasons" "Ambient"

# 2. Fast arpeggio, MP3 320 kbps CBR 44.1 kHz.
build arp_prog 0.125 0.12
encode 02-glass-arpeggio.mp3 30 "$(stereo_src 30 44100)" \
    "-c:a libmp3lame -b:a 320k -id3v2_version 3" \
    "Glass Arpeggio" "Lumen Harbor" "Synthetic Seasons" "Ambient"

# 3. Kick and bass pattern, Ogg Vorbis 48 kHz.
build kick_pat 0.9
encode 03-low-thump.ogg 30 "$(stereo_src 30 48000)" "-c:a libvorbis -q:a 5" \
    "Low Thump" "Subterra Club" "Pulse Tests" "Electronic"

# 4. Filtered noise hi-hat, WAV 16-bit 44.1 kHz. Closed hat every 0.25 s, open hat off the beat.
hat_env="0.8*exp(-30*mod(t,0.25))+0.5*exp(-9*mod(t+0.25,0.5))"
hat_src="anoisesrc=d=30:c=white:r=44100:a=0.6:seed=7,highpass=f=7000,volume=eval=frame:volume=$(esc "$hat_env"),aformat=channel_layouts=stereo"
encode 04-hat-and-noise.wav 30 "$hat_src" "-c:a pcm_s16le" \
    "Hat and Noise" "Subterra Club" "Pulse Tests" "Electronic"

# 5. Logarithmic sweep from 20 Hz to 20 kHz, FLAC 24-bit 96 kHz.
sweep="0.5*sin(2*PI*20*20/log(1000)*(pow(1000,t/20)-1))"
EL=$sweep ER=$sweep
encode 05-sweep-20hz-20khz.flac 20 "$(stereo_src 20 96000)" \
    "-c:a flac -sample_fmt s32 -bits_per_raw_sample 24" \
    "Sweep 20 Hz to 20 kHz" "Test Signal Orchestra" "Calibration" "Test"

# 6. A very quiet passage (about 30 dB below full scale), AIFF 16-bit 44.1 kHz.
build pad_prog 0.1
SCALE=0.06
encode 06-silencio-en-el-café.aiff 45 "$(stereo_src 45 44100)" "-c:a pcm_s16be -write_id3v2 1" \
    "Silencio en el Café" "Orquesta Sintética del Sur" "Canciones de Cuna Sintéticas" "Folk"
SCALE=1

# 7. A long track of 4 minutes, MP3 VBR 44.1 kHz.
pad_arp() { pad_prog 0.07; arp_prog 0.25 0.07; }
build pad_arp
encode 07-long-evening.mp3 240 "$(stereo_src 240 44100)" "-c:a libmp3lame -q:a 2 -id3v2_version 3" \
    "Long Evening" "Marrow Field" "Long Players" "Ambient"

# 8. A waltz, WAV 24-bit 48 kHz.
build waltz
encode 08-valse-électrique.wav 36 "$(stereo_src 36 48000)" "-c:a pcm_s24le" \
    "Valse Électrique" "Zoë Quartz" "Études Électriques" "Classical"

# 9. Kick, pad and arpeggio together, M4A (AAC) 48 kHz.
mix() { kick_pat 0.5; pad_prog 0.06; arp_prog 0.25 0.07; }
build mix
encode 09-pulse-and-pad.m4a 40 "$(stereo_src 40 48000)" "-c:a aac -b:a 192k" \
    "Pulse and Pad" "Subterra Club" "Pulse Tests" "Electronic"

# 10. A small tune, mono MP3 22.05 kHz.
mono_tune() { melody; pad_prog 0.04; }
build mono_tune
encode 10-radio-de-onda-corta.mp3 24 \
    "aevalsrc=exprs=$(esc "clip($EL,-0.95,0.95)"):s=22050:d=24:c=mono" \
    "-c:a libmp3lame -b:a 64k -ar 22050 -ac 1 -id3v2_version 3" \
    "Radio de Onda Corta" "Orquesta Sintética del Sur" "Canciones de Cuna Sintéticas" "Folk"

# 11. Notes that bounce between the channels (balance check), Ogg Vorbis 44.1 kHz.
PAR=0
pingpong
compose
EL=$E
PAR=1
pingpong
compose
ER=$E
encode 11-ping-pong.ogg 32 "$(stereo_src 32 44100)" "-c:a libvorbis -q:a 4" \
    "Ping Pong" "Marrow Field" "Long Players" "Electronic"

# 12. A ladder of tones, one per equalizer band (31 Hz to 16 kHz, 2 s each), then all together.
TT=t
i=0
for f in 31 62 125 250 500 1000 2000 4000 8000 16000; do
    tone "f$f" $((i * 2)) 2 0.45 "$f"
    tone "f$f" 20 2 0.045 "$f"
    i=$((i + 1))
done
compose
EL=$E ER=$E
encode 12-band-ladder.flac 22 "$(stereo_src 22 44100)" "-c:a flac -sample_fmt s16" \
    "Band Ladder" "Test Signal Orchestra" "Calibration" "Test"

# The playlist, in order, with the duration of each file.
list=$out/sample.m3u8
printf '#EXTM3U\n' > "$list"
for entry in "${TRACKS[@]}"; do
    IFS='|' read -r file artist title <<< "$entry"
    secs=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$out/$file")
    printf '#EXTINF:%d,%s - %s\n%s\n' "$(awk -v s="$secs" 'BEGIN { printf "%d", s + 0.5 }')" \
        "$artist" "$title" "$file" >> "$list"
done
echo "Wrote $list ($N tracks)"
