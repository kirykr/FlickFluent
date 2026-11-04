#!/bin/sh
# Builds a 3-hour file by looping the 2-minute clip 90 times (stream copy, no re-encode).
set -eu
cd "$(dirname "$0")"
ffmpeg -v error -y -stream_loop 89 -i clip-2min-fr.mp4 -c copy long-3h-fr.mp4
ffprobe -v error -show_entries format=duration -of csv=p=0 long-3h-fr.mp4
