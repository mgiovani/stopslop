#!/usr/bin/env bash
# Before/after benchmark for perf PRs. Paste the tables into the PR body.
#
#   bench/run.sh <base-binary> <new-binary> [inputs-dir]
#
# Inputs are generated into target/bench-inputs by bench/gen_inputs.py when the
# directory is omitted. Set STOPSLOP_BENCH_CORPUS=<dir> to add a multi-file run.
set -euo pipefail

base=$(realpath "$1")
new=$(realpath "$2")
root=$(cd "$(dirname "$0")/.." && pwd)
inputs=${3:-"$root/target/bench-inputs"}
[ -f "$inputs/headings_20mb.md" ] || python3 "$root/bench/gen_inputs.py" "$inputs" >/dev/null
export STOPSLOP_NO_UPDATE_CHECK=1 CI=1
cd "$root"

bench() {
  local title=$1 runs=$2 flags=$3; shift 3
  local md; md=$(mktemp)
  hyperfine -i -r "$runs" $flags --time-unit millisecond --export-markdown "$md" \
    -n "base: $title" "$base $*" -n "new: $title" "$new $*" >/dev/null
  tail -n +3 "$md"
  command rm -f "$md"
}

rss() {
  case $(uname) in
    Darwin) /usr/bin/time -l "$@" 2>&1 >/dev/null | awk '/maximum resident/ {printf "%.0f MB", $1/1048576}' ;;
    *) /usr/bin/time -v "$@" 2>&1 >/dev/null | awk '/Maximum resident/ {printf "%.0f MB", $NF/1024}' ;;
  esac
}

echo "| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |"
echo "|:---|---:|---:|---:|---:|"
bench "three_lines.md" 30 "-N -w 5" --no-config --format json "$inputs/three_lines.md"
bench "self-lint" 20 "-N -w 3" --format json .
for f in prose_8mb_emdash prose_8mb_ascii headings_20mb oneline_700k; do
  bench "$f.md" 5 "-w 1" --no-config --format json "$inputs/$f.md"
done
if [ -n "${STOPSLOP_BENCH_CORPUS:-}" ]; then
  bench "corpus ALL" 5 "-w 1" --no-config --select ALL --format json "$STOPSLOP_BENCH_CORPUS"
fi

echo
echo "| Peak RSS on headings_20mb.md | base | new |"
echo "|:---|---:|---:|"
echo "| default rules, json | $(rss "$base" --no-config --format json "$inputs/headings_20mb.md") | $(rss "$new" --no-config --format json "$inputs/headings_20mb.md") |"
