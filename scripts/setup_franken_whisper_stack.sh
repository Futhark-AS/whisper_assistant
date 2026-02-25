#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
STACK_ROOT="${FRANKEN_STACK_ROOT:-${REPO_ROOT}/third_party/franken-stack}"

REPO_NAMES=(
  "franken_whisper"
  "asupersync"
  "frankensqlite"
  "frankentui"
  "frankentorch"
  "frankenjax"
)

REPO_URLS=(
  "https://github.com/Dicklesworthstone/franken_whisper"
  "https://github.com/Dicklesworthstone/asupersync.git"
  "https://github.com/Dicklesworthstone/frankensqlite"
  "https://github.com/Dicklesworthstone/frankentui.git"
  "https://github.com/Dicklesworthstone/frankentorch.git"
  "https://github.com/Dicklesworthstone/frankenjax.git"
)

REPO_COMMITS=(
  "34d7bc213b94ede0ca7dbf1dca1fac359fe5d365"
  "b512b91f016e892d91cd1fdade200c7042bc02c7"
  "c89e82bc1f93b727988f81f5c5dfa8ede067adf4"
  "78b29e865575ee96045f27c378ae231e018dc861"
  "0bc2cdb087611d1601b81ee06ea820de4532b455"
  "ff8e856c0f7362cc55e6d3a2315aa95654ebb431"
)

required_fail=0

declare -a MATRIX

green() { printf '\033[32m%s\033[0m' "$1"; }
red() { printf '\033[31m%s\033[0m' "$1"; }
yellow() { printf '\033[33m%s\033[0m' "$1"; }

add_matrix() {
  local name="$1"
  local status="$2"
  local detail="$3"
  MATRIX+=("${name}|${status}|${detail}")
}

version_from_text() {
  local raw="$1"
  local version
  version="$(printf '%s' "$raw" | grep -Eo '[0-9]+(\.[0-9]+){1,3}' | head -n1 || true)"
  printf '%s' "$version"
}

version_ge() {
  local have="$1"
  local want="$2"
  [[ -n "$have" ]] || return 1
  local sorted
  sorted="$(printf '%s\n%s\n' "$want" "$have" | sort -V | head -n1)"
  [[ "$sorted" == "$want" ]]
}

check_binary_with_min() {
  local binary="$1"
  local min_version="$2"
  local required="$3"

  if ! command -v "$binary" >/dev/null 2>&1; then
    add_matrix "$binary" "FAIL" "missing from PATH"
    if [[ "$required" == "required" ]]; then
      required_fail=1
    fi
    return
  fi

  local output=""
  output="$($binary --version 2>/dev/null || true)"
  if [[ -z "$output" ]]; then
    output="$($binary -V 2>/dev/null || true)"
  fi
  if [[ -z "$output" ]]; then
    output="$($binary version 2>/dev/null || true)"
  fi

  local parsed
  parsed="$(version_from_text "$output")"
  if [[ -z "$parsed" ]]; then
    add_matrix "$binary" "WARN" "installed, but version parse failed"
    return
  fi

  if version_ge "$parsed" "$min_version"; then
    add_matrix "$binary" "PASS" "${parsed} (>= ${min_version})"
  else
    add_matrix "$binary" "FAIL" "${parsed} (< ${min_version})"
    if [[ "$required" == "required" ]]; then
      required_fail=1
    fi
  fi
}

clone_or_update_repo() {
  local name="$1"
  local url="$2"
  local commit="$3"
  local target="${STACK_ROOT}/${name}"

  if [[ ! -d "$target/.git" ]]; then
    git clone "$url" "$target"
  fi

  (
    cd "$target"
    git fetch --all --tags --prune
    git checkout --detach "$commit"
  )

  add_matrix "repo:${name}" "PASS" "checked out ${commit}"
}

check_toolchain() {
  if ! command -v rustup >/dev/null 2>&1; then
    add_matrix "rustup" "FAIL" "rustup not found"
    required_fail=1
  else
    add_matrix "rustup" "PASS" "$(rustup --version | head -n1)"
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    add_matrix "cargo" "FAIL" "cargo not found"
    required_fail=1
  else
    add_matrix "cargo" "PASS" "$(cargo --version)"
  fi

  if ! command -v rustc >/dev/null 2>&1; then
    add_matrix "rustc" "FAIL" "rustc not found"
    required_fail=1
  else
    add_matrix "rustc" "PASS" "$(rustc --version)"
  fi
}

run_franken_check() {
  local fw_dir="${STACK_ROOT}/franken_whisper"
  if [[ ! -f "${fw_dir}/Cargo.toml" ]]; then
    add_matrix "franken_whisper:cargo-check" "FAIL" "Cargo.toml not found"
    required_fail=1
    return
  fi

  if (cd "$fw_dir" && cargo check); then
    add_matrix "franken_whisper:cargo-check" "PASS" "cargo check succeeded"
  else
    add_matrix "franken_whisper:cargo-check" "FAIL" "cargo check failed"
    required_fail=1
  fi
}

run_macos_metal_checks() {
  if [[ "$(uname -s)" != "Darwin" ]]; then
    add_matrix "metal-check" "SKIP" "not macOS"
    return
  fi

  if ! command -v whisper-cli >/dev/null 2>&1; then
    add_matrix "metal-check" "FAIL" "whisper-cli missing"
    required_fail=1
    return
  fi

  local whisper_path
  whisper_path="$(command -v whisper-cli)"
  if otool -L "$whisper_path" | grep -q 'Metal.framework'; then
    add_matrix "metal-link" "PASS" "Metal.framework linked"
  else
    add_matrix "metal-link" "FAIL" "Metal.framework not linked"
    required_fail=1
  fi

  local model_path="${WHISPER_MODEL_PATH:-$HOME/.cache/whisper.cpp/ggml-base.en.bin}"
  if [[ ! -r "$model_path" ]]; then
    add_matrix "whisper-model" "FAIL" "model missing/readable: ${model_path}"
    required_fail=1
    return
  fi
  add_matrix "whisper-model" "PASS" "$model_path"

  if ! command -v ffmpeg >/dev/null 2>&1; then
    add_matrix "metal-smoke" "FAIL" "ffmpeg required for smoke test audio"
    required_fail=1
    return
  fi

  local tmpdir
  tmpdir="$(mktemp -d)"
  local wav_file="${tmpdir}/smoke.wav"
  local log_file="${tmpdir}/whisper.log"

  ffmpeg -hide_banner -loglevel error -f lavfi -i anullsrc=r=16000:cl=mono -t 1 "$wav_file"

  if whisper-cli -m "$model_path" -f "$wav_file" -l en -otxt -of "${tmpdir}/out" >"$log_file" 2>&1; then
    if grep -Eiq 'metal|mps|gpu' "$log_file"; then
      add_matrix "metal-smoke" "PASS" "metal markers found in whisper-cli logs"
    else
      add_matrix "metal-smoke" "WARN" "smoke test passed but no metal marker found"
    fi
  else
    add_matrix "metal-smoke" "FAIL" "whisper-cli smoke test failed"
    required_fail=1
  fi

  rm -rf "$tmpdir"
}

print_matrix() {
  printf '\nSetup Verification Matrix\n'
  printf '%-34s %-8s %s\n' "CHECK" "STATUS" "DETAIL"
  printf '%-34s %-8s %s\n' "-----" "------" "------"

  local row name status detail
  for row in "${MATRIX[@]}"; do
    IFS='|' read -r name status detail <<<"$row"
    case "$status" in
      PASS) status="$(green PASS)" ;;
      FAIL) status="$(red FAIL)" ;;
      WARN) status="$(yellow WARN)" ;;
      SKIP) status="$(yellow SKIP)" ;;
      *) ;;
    esac
    printf '%-34s %-17s %s\n' "$name" "$status" "$detail"
  done

  printf '\nRemediation Commands\n'
  printf '%s\n' '  macOS (Homebrew): brew install ffmpeg whisper-cpp'
  printf '%s\n' '  Linux (Debian/Ubuntu): sudo apt-get install -y ffmpeg'
  printf '%s\n' '  whisper-cli: build from https://github.com/ggml-org/whisper.cpp'
  printf '%s\n' '  insanely-fast-whisper: pipx install insanely-fast-whisper'
  printf '%s\n' '  python3 >= 3.10: install from system package manager'
}

main() {
  mkdir -p "$STACK_ROOT"

  printf 'Using stack root: %s\n' "$STACK_ROOT"

  local i
  for i in "${!REPO_NAMES[@]}"; do
    clone_or_update_repo "${REPO_NAMES[$i]}" "${REPO_URLS[$i]}" "${REPO_COMMITS[$i]}"
  done

  check_toolchain
  run_franken_check

  check_binary_with_min "ffmpeg" "6.0" "required"
  check_binary_with_min "ffprobe" "6.0" "required"
  check_binary_with_min "whisper-cli" "1.7.2" "required"
  check_binary_with_min "insanely-fast-whisper" "0.0.15" "optional"
  check_binary_with_min "python3" "3.10" "optional"

  run_macos_metal_checks

  print_matrix

  if [[ "$required_fail" -ne 0 ]]; then
    printf '\nRequired checks failed.\n' >&2
    exit 1
  fi

  printf '\nAll required checks passed.\n'
}

main "$@"
