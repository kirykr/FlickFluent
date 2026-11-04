#!/bin/sh
# Builds the sample clips with macOS `say` and ffmpeg. All audio is self-made, so no third-party licence applies.
# Needs: macOS (say), ffmpeg. The generated clips are committed, so Windows does not need to rebuild them.
set -eu
cd "$(dirname "$0")"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

music() { # $1 seconds $2 out
  ffmpeg -v error -y -f lavfi -i "sine=frequency=220:duration=$1" -f lavfi -i "sine=frequency=330:duration=$1" \
    -filter_complex "[0][1]amix=inputs=2,volume=0.25" -ar 44100 -ac 2 "$2"
}
speak() { # $1 voice $2 text $3 out
  say -v "$1" -o "$tmp/s.aiff" "$2"
  ffmpeg -v error -y -i "$tmp/s.aiff" -ar 44100 -ac 2 "$3"
}
video() { # $1 audio $2 out
  d=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$1")
  ffmpeg -v error -y -f lavfi -i "color=c=navy:s=320x240:r=15:d=$d" -i "$1" -shortest -c:v libx264 -pix_fmt yuv420p -c:a aac "$2"
}

# 20 s, two speakers (Thomas fr, Samantha en would break "one language"; use two French voices)
speak Thomas "Bonjour, je m'appelle Thomas. Aujourd'hui nous allons visiter la ville." "$tmp/a.wav"
speak Amelie "Merci beaucoup. J'ai hâte de voir le grand musée." "$tmp/b.wav" 2>/dev/null || speak Thomas "Merci beaucoup. J'ai hâte de voir le grand musée." "$tmp/b.wav"
ffmpeg -v error -y -f lavfi -i anullsrc=r=44100:cl=stereo -t 1.5 "$tmp/gap.wav"
ffmpeg -v error -y -i "$tmp/a.wav" -i "$tmp/gap.wav" -i "$tmp/b.wav" -i "$tmp/gap.wav" -i "$tmp/a.wav" \
  -filter_complex "concat=n=5:v=0:a=1,apad=whole_dur=20" -t 20 "$tmp/speech20.wav"
music 20 "$tmp/m20.wav"
ffmpeg -v error -y -i "$tmp/speech20.wav" -i "$tmp/m20.wav" -filter_complex "amix=inputs=2:normalize=0" -t 20 "$tmp/mix20.wav"
video "$tmp/mix20.wav" clip-20s-fr.mp4

# music only, 10 s
music 10 "$tmp/m10.wav"; video "$tmp/m10.wav" clip-music-only.mp4
# speech-free (silence), 10 s
ffmpeg -v error -y -f lavfi -i anullsrc=r=44100:cl=stereo -t 10 "$tmp/sil.wav"; video "$tmp/sil.wav" clip-no-speech.mp4
# silent video (no audio track), 5 s
ffmpeg -v error -y -f lavfi -i "color=c=black:s=320x240:r=15:d=5" -c:v libx264 -pix_fmt yuv420p -an clip-no-audio.mp4
# corrupt file
head -c 4096 /dev/urandom > clip-corrupt.mp4
# 2-minute clip: the 20 s clip looped 6 times
ffmpeg -v error -y -stream_loop 5 -i clip-20s-fr.mp4 -c copy clip-2min-fr.mp4
echo done
