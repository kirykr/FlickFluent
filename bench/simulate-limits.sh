#!/usr/bin/env bash
# Run the benchmark as if the computer were the minimum spec (macOS).
#
#   bench/simulate-limits.sh [--cores 4] [--ram-gb 8] [--bin <flickfluent>] [-- <bench options>]
#
# What it can and cannot do on a Mac (the record says `simulated` either way):
#   * Models and threads: the app picks the minimum-class models and gives the workers cores-1
#     threads, exactly as it would on a machine with that many cores (--simulate-cores/-ram-gb).
#   * Memory: macOS has no hard per-process cap that works for this, so a watchdog adds up the
#     resident memory of the app and everything it started once a second and stops the run if it
#     passes the RAM limit. A run that passes is proven to fit; a stopped one is proven not to.
#   * Not simulated: a real 8 GB machine also runs the operating system and other apps, and its
#     CPU is slower than this one. Speed numbers from a simulated run are a best case.
set -euo pipefail

cores=4
ram_gb=8
bin="$(cd "$(dirname "$0")/.." && pwd)/target/debug/flickfluent"
while [ $# -gt 0 ]; do
  case "$1" in
    --cores) cores="$2"; shift 2 ;;
    --ram-gb) ram_gb="$2"; shift 2 ;;
    --bin) bin="$2"; shift 2 ;;
    --) shift; break ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
[ -x "$bin" ] || { echo "no program at $bin (build it, or pass --bin)" >&2; exit 2; }

limit_kb=$(awk -v g="$ram_gb" 'BEGIN { printf "%d", g * 1024 * 1024 }')

"$bin" --bench --simulate-cores "$cores" --simulate-ram-gb "$ram_gb" "$@" &
root=$!

# Resident memory of the process and all its descendants, in KB.
tree_kb() {
  ps -A -o pid=,ppid=,rss= | awk -v root="$root" '
    { ppid[$1] = $2; rss[$1] = $3 }
    END {
      for (p in ppid) {
        q = p
        while (q != "" && q != 0) { if (q == root) { sum += rss[p]; break }; q = ppid[q] }
      }
      print sum + 0
    }'
}

peak=0
exceeded=0
while kill -0 "$root" 2>/dev/null; do
  now=$(tree_kb)
  [ "$now" -gt "$peak" ] && peak=$now
  if [ "$now" -gt "$limit_kb" ]; then
    exceeded=1
    pkill -TERM -P "$root" 2>/dev/null || true
    kill -TERM "$root" 2>/dev/null || true
    break
  fi
  sleep 1
done
status=0
wait "$root" || status=$?
echo "watchdog: peak resident memory of the whole run $((peak / 1024)) MB, limit $((limit_kb / 1024)) MB" >&2
if [ "$exceeded" -eq 1 ]; then
  echo "watchdog: the run passed the ${ram_gb} GB limit and was stopped, so this computer size is NOT enough" >&2
  exit 137
fi
exit "$status"
