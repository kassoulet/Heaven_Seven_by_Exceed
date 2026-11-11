#!/bin/bash
# Offline render of the intro to an mp4 with the music.  Usage: tools/mkvideo.sh 320|1920 [fps] [full]
#   320  : the original's exact 320x240 mode (bit-exact, 25 fps grid), letterbox cropped -> 320x176
#   1920 : native 1920x1080 render (16:9, no letterbox); fps default 60 = finer game-time step (--real), not bit-exact
#   full : (env H7FULL=1) trace every pixel instead of the original's adaptive subdivision (needs the full-ray switch in h7core)
# Output: ../h7-videos/ (outside git; the wav is cached there).
set -euo pipefail
cd "$(dirname "$0")/.."
G=${CARGO_TARGET_DIR:-$HOME/target}/release/h7gen
[ -x "$G" ] || { echo "build first: cargo build --release"; exit 1; }
OUT=../h7-videos; mkdir -p "$OUT"
case "${1:-}" in
  320)  W=320;  H=240;  FPS=${2:-25}; CROP="-vf crop=iw:176:0:0"; MODE="" ;;
  1920) W=1920; H=1080; FPS=${2:-60}; CROP=""; MODE="--real" ;;
  *) echo "usage: $0 320|1920 [fps] [full]"; exit 2 ;;
esac
[ "$FPS" = 25 ] && MODE=""          # 25 fps = the original's own 40 ms grid
[ "${3:-}" = full ] && { export H7FULL=1; TAG=_full"${TAG:-}"; }
WAV=$OUT/h7.wav; [ -f "$WAV" ] || "$G" --audio "$WAV"
[ "$W" = 320 ] || export H7SIZE=${W}x${H}
"$G" $MODE --fps "$FPS" --size ${W}x${H} --out - |
  ffmpeg -y -loglevel error -f rawvideo -pix_fmt rgba -s ${W}x${H} -r "$FPS" -i - -i "$WAV" $CROP \
    -c:v libx264 -preset medium -crf 20 -pix_fmt yuv420p -movflags +faststart -c:a aac -b:a 128k -shortest \
    "$OUT/h7_${W}${TAG:-}_${FPS}fps.mp4"
echo "$OUT/h7_${W}${TAG:-}_${FPS}fps.mp4"
