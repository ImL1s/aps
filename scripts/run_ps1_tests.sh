#!/usr/bin/env bash
# ==============================================================================
# PlayStation 1 (MIPS R3000A) Test ROM Automation Harness Script
# ==============================================================================
# Script: scripts/run_ps1_tests.sh
# Target: aps PS1 Emulator E2E Test Suite (Amidog psxtest_cpu.exe)
# ==============================================================================

set -euo pipefail

# ANSI Color Output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

# Paths
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROM_DIR="${PROJECT_ROOT}/tests/roms"
BIOS_DIR="${PROJECT_ROOT}/bios"
TARGET_BIN="${PROJECT_ROOT}/target/release/aps"
if [[ -f "${TARGET_BIN}.exe" ]]; then
  TARGET_BIN="${TARGET_BIN}.exe"
fi
AMIDOG_ROM="${ROM_DIR}/psxtest_cpu.exe"
AMIDOG_ZIP_URL="https://psx.amidog.se/lib/exe/fetch.php?media=psx:download:psxtest_cpu.zip"

# Options
DOWNLOAD_ONLY=false
FORCE_DOWNLOAD=false
SKIP_BUILD=false
HEADLESS=true
CUSTOM_TIMEOUT=60
MAX_CYCLES=100000000

# Parse Arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --download-only)
      DOWNLOAD_ONLY=true
      shift
      ;;
    --force-download)
      FORCE_DOWNLOAD=true
      shift
      ;;
    --no-build)
      SKIP_BUILD=true
      shift
      ;;
    --timeout)
      if [[ -n "${2:-}" ]] && [[ "$2" =~ ^[0-9]+$ ]]; then
        CUSTOM_TIMEOUT="$2"
        shift 2
      else
        echo -e "${RED}Error: --timeout requires a numeric argument (seconds)${NC}" >&2
        exit 1
      fi
      ;;
    --max-cycles)
      if [[ -n "${2:-}" ]] && [[ "$2" =~ ^[0-9]+$ ]]; then
        MAX_CYCLES="$2"
        shift 2
      else
        echo -e "${RED}Error: --max-cycles requires a numeric argument${NC}" >&2
        exit 1
      fi
      ;;
    --help|-h)
      echo -e "${BOLD}APS PS1 Emulator Test Harness usage:${NC}"
      echo -e "  $0 [OPTIONS]"
      echo -e ""
      echo -e "${BOLD}Options:${NC}"
      echo -e "  --download-only   Download/verify test ROM without running tests"
      echo -e "  --force-download  Bypass cache and force re-download of test ROM"
      echo -e "  --no-build        Skip 'cargo build --release' step"
      echo -e "  --timeout <N>     Execution timeout limit in seconds (default: 60)"
      echo -e "  --max-cycles <N>  Max CPU cycle limit for headless run (default: 100000000)"
      echo -e "  --help, -h        Display this help message"
      exit 0
      ;;
    *)
      echo -e "${RED}Error: Unknown option $1${NC}" >&2
      exit 1
      ;;
  esac
done

mkdir -p "${ROM_DIR}"
mkdir -p "${BIOS_DIR}"

echo -e "${BOLD}${BLUE}================================================================${NC}"
echo -e "${BOLD}${BLUE}   PlayStation 1 Emulator Test Harness (Amidog CPU Test)       ${NC}"
echo -e "${BOLD}${BLUE}================================================================${NC}"

# ------------------------------------------------------------------------------
# Step 1: Download / Cache Verification for Amidog psxtest_cpu.exe
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[Step 1/4] Verifying Test ROM Cache (${AMIDOG_ROM#${PROJECT_ROOT}/})${NC}"

get_file_size() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    echo "0"
    return 0
  fi
  local sz
  sz=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null || wc -c < "$file" 2>/dev/null || echo "0")
  echo "${sz//[^0-9]/}"
}

rom_valid=false
if [[ -f "${AMIDOG_ROM}" ]] && [[ "${FORCE_DOWNLOAD}" == false ]]; then
  file_size=$(get_file_size "${AMIDOG_ROM}")
  if [[ "${file_size}" -gt 10000 ]]; then
    rom_valid=true
    echo -e "[ ${GREEN}CACHED${NC} ] ${AMIDOG_ROM#${PROJECT_ROOT}/} (${file_size} bytes)"
  fi
fi

if [[ "${rom_valid}" == false ]]; then
  echo -e "[ ${YELLOW}DOWNLOADING${NC} ] Fetching psxtest_cpu.zip from remote repository..."
  download_success=false
  tmp_zip="${ROM_DIR}/psxtest_cpu_download.zip"
  rm -f "${tmp_zip}"

  if command -v curl &>/dev/null; then
    if curl -sSL --fail -A "Mozilla/5.0" --connect-timeout 15 --max-time 60 "${AMIDOG_ZIP_URL}" -o "${tmp_zip}"; then
      download_success=true
    fi
  elif command -v wget &>/dev/null; then
    if wget -q -U "Mozilla/5.0" --timeout=60 "${AMIDOG_ZIP_URL}" -O "${tmp_zip}"; then
      download_success=true
    fi
  fi

  if [[ "${download_success}" == true ]] && [[ -f "${tmp_zip}" ]]; then
    echo -e "[ ${GREEN}EXTRACTING${NC} ] Unpacking psxtest_cpu.exe from zip archive..."
    if command -v unzip &>/dev/null; then
      unzip -o -q "${tmp_zip}" -d "${ROM_DIR}" 2>/dev/null || true
    elif command -v python3 &>/dev/null; then
      python3 -m zipfile -e "${tmp_zip}" "${ROM_DIR}" 2>/dev/null || true
    fi
    rm -f "${tmp_zip}"
  fi

  file_size=$(get_file_size "${AMIDOG_ROM}")
  if [[ "${file_size}" -gt 10000 ]]; then
    echo -e "[ ${GREEN}DOWNLOADED${NC} ] Successfully cached psxtest_cpu.exe (${file_size} bytes)"
  else
    if [[ -f "${AMIDOG_ROM}" ]] && [[ $(get_file_size "${AMIDOG_ROM}") -gt 10000 ]]; then
      echo -e "[ ${YELLOW}WARN${NC} ] Network download failed; falling back to existing local file."
    else
      echo -e "${RED}[ ERROR ] Download failed and no valid local ROM cache found.${NC}" >&2
      echo -e "${YELLOW}Action Required: Please place 'psxtest_cpu.exe' manually into 'tests/roms/' directory.${NC}" >&2
      exit 3
    fi
  fi
fi

if [[ "${DOWNLOAD_ONLY}" == true ]]; then
  echo -e "\n${GREEN}Download verification completed successfully. (--download-only mode)${NC}"
  exit 0
fi

# ------------------------------------------------------------------------------
# Step 2: Build Release Binary
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[Step 2/4] Building Emulator Release Binary (${TARGET_BIN#${PROJECT_ROOT}/})${NC}"

if [[ "${SKIP_BUILD}" == true ]]; then
  if [[ ! -x "${TARGET_BIN}" ]]; then
    echo -e "${RED}Error: --no-build specified but release binary '${TARGET_BIN}' not found or not executable.${NC}" >&2
    exit 2
  fi
  echo -e "[ ${YELLOW}SKIPPED${NC} ] Using pre-built release binary."
else
  if cargo build --release --manifest-path "${PROJECT_ROOT}/Cargo.toml"; then
    echo -e "[ ${GREEN}BUILD SUCCESS${NC} ] Cargo release binary ready."
    if [[ -f "${PROJECT_ROOT}/target/release/aps.exe" ]]; then
      TARGET_BIN="${PROJECT_ROOT}/target/release/aps.exe"
    elif [[ "$(uname)" == "Darwin" ]]; then
      brew_prefix="$(brew --prefix 2>/dev/null || echo /opt/homebrew)"
      if [[ -d "${brew_prefix}/lib" ]]; then
        install_name_tool -add_rpath "${brew_prefix}/lib" "${TARGET_BIN}" 2>/dev/null || true
      fi
      if [[ -d "/usr/local/lib" ]]; then
        install_name_tool -add_rpath "/usr/local/lib" "${TARGET_BIN}" 2>/dev/null || true
      fi
    fi
  else
    echo -e "${RED}[ ERROR ] Cargo release build failed.${NC}" >&2
    exit 2
  fi
fi

# ------------------------------------------------------------------------------
# Step 3: Execute Headless Test ROM with Timeout Control
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[Step 3/4] Running Headless Test Suite (Timeout: ${CUSTOM_TIMEOUT}s, Max Cycles: ${MAX_CYCLES})${NC}"

LOG_FILE=$(mktemp "${TMPDIR:-/tmp}/psx_amidog_test_XXXXXX")
trap 'rm -f "${LOG_FILE}"' EXIT INT TERM

run_with_timeout() {
  local sec="$1"
  local log="$2"
  shift 2

  if command -v timeout &>/dev/null; then
    timeout "${sec}s" "$@" > "${log}" 2>&1
    return $?
  elif command -v gtimeout &>/dev/null; then
    gtimeout "${sec}s" "$@" > "${log}" 2>&1
    return $?
  else
    # Portable POSIX subshell background runner with PID watcher
    "$@" > "${log}" 2>&1 &
    local pid=$!
    local count=0
    while kill -0 $pid 2>/dev/null; do
      if [[ $count -ge $sec ]]; then
        kill -9 $pid 2>/dev/null || true
        wait $pid 2>/dev/null || true
        echo "ERROR: Process timed out after ${sec} seconds" >> "${log}"
        return 124
      fi
      sleep 1
      ((count++))
    done
    wait $pid
    return $?
  fi
}

start_time=$(date +%s)
exit_code=0
run_with_timeout "${CUSTOM_TIMEOUT}" "${LOG_FILE}" "${TARGET_BIN}" --headless --max-cycles "${MAX_CYCLES}" "${AMIDOG_ROM}" || exit_code=$?
end_time=$(date +%s)
duration=$((end_time - start_time))

# ------------------------------------------------------------------------------
# Step 4: Parse Verification Output & Report Exit Code
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[Step 4/4] Parsing Verification Output${NC}"
echo -e "----------------------------------------------------------------"
cat "${LOG_FILE}"
echo -e "----------------------------------------------------------------"

if [[ $exit_code -eq 124 ]]; then
  echo -e "\n${RED}================================================================${NC}"
  echo -e "${RED}  FAILURE: Test execution TIMED OUT after ${CUSTOM_TIMEOUT} seconds (Exit Code 124)${NC}"
  echo -e "${RED}================================================================${NC}\n"
  exit 124
fi

pass_match=false
if grep -q -E -i "All tests done: 00000000|All tests done: 0|All tests passed|101/101|101 tests passed" "${LOG_FILE}"; then
  pass_match=true
elif [[ $(grep -c -E "\bPASS\b" "${LOG_FILE}" || true) -ge 101 ]]; then
  pass_match=true
fi

has_failure=false
if grep -E -i "\bFAIL(ED)?\b|CPU Panic|Unhandled Exception|error @" "${LOG_FILE}" | grep -v -E -i "FAILED: 0|FAILED: 00000000|0 failed" | grep -q .; then
  has_failure=true
fi

if [[ "${pass_match}" == true ]] && [[ "${has_failure}" == false ]] && [[ $exit_code -eq 0 ]]; then
  echo -e "\n${GREEN}================================================================${NC}"
  echo -e "${GREEN}  SUCCESS: Amidog PS1 CPU Test Suite PASSED! (${duration}s)${NC}"
  echo -e "${GREEN}================================================================${NC}\n"
  exit 0
else
  echo -e "\n${RED}================================================================${NC}"
  echo -e "${RED}  FAILURE: Amidog PS1 CPU Test Suite Verification Failed! (Exit Code 1)${NC}"
  echo -e "${RED}================================================================${NC}\n"
  exit 1
fi
