#!/usr/bin/env bash
# check-all.sh — Runs all project checks
# Variant: PURE_RUST — regenerate with: cc-init-checks --force
# Usage: ./check-all.sh [--rust-only | --frontend-only]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOGS_DIR="${SCRIPT_DIR}/logs"
mkdir -p "${LOGS_DIR}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

FAILED_CHECKS=()

run_check() {
  local label="$1"
  local logfile="${LOGS_DIR}/${2}"
  shift 2
  local cmd=("$@")
  printf "  %-35s" "${label}..."
  if "${cmd[@]}" > "${logfile}" 2>&1; then
    echo -e " ${GREEN}OK${NC}"
  else
    local err_count
    err_count=$(grep -cE '(error|Error|FAILED)' "${logfile}" 2>/dev/null || true)
    echo -e " ${RED}FAIL${NC}  (${err_count} errors) → logs/${logfile##*/}"
    FAILED_CHECKS+=("${label}")
  fi
}

run_check_in() {
  local label="$1"
  local logfile="${LOGS_DIR}/${2}"
  local dir="$3"
  shift 3
  local cmd=("$@")
  printf "  %-35s" "${label}..."
  if (cd "${dir}" && "${cmd[@]}" > "${logfile}" 2>&1); then
    echo -e " ${GREEN}OK${NC}"
  else
    local err_count
    err_count=$(grep -cE '(error|Error|FAILED)' "${logfile}" 2>/dev/null || true)
    echo -e " ${RED}FAIL${NC}  (${err_count} errors) → logs/${logfile##*/}"
    FAILED_CHECKS+=("${label}")
  fi
}

run_if_installed() {
  local binary="$1" label="$2" logfile="${3}"; shift 3
  command -v "$binary" > /dev/null 2>&1 || return 0
  run_check "$label" "$logfile" "$@"
}

run_if_installed_in() {
  local binary="$1" label="$2" logfile="${3}" dir="$4"; shift 4
  command -v "$binary" > /dev/null 2>&1 || return 0
  run_check_in "$label" "$logfile" "$dir" "$@"
}

MODE="${1:-all}"

echo -e "\n${YELLOW}── Rust${NC}"
run_check_in "cargo fmt --check"  "cargo-fmt.log"    "${SCRIPT_DIR}"  cargo fmt --check
run_check_in "cargo clippy"       "cargo-clippy.log" "${SCRIPT_DIR}"  cargo clippy -- -D warnings
run_check_in "cargo test"         "cargo-test.log"   "${SCRIPT_DIR}"  cargo test

echo -e "\n${YELLOW}── Security${NC}"
run_if_installed "cargo-audit" "cargo audit" "rustsec.log"  cargo audit

echo -e "\n${YELLOW}── Extras${NC}"
if [[ "${MODE}" == "all" ]]; then
  run_if_installed "typos" "typos" "typos.log"  typos .
fi

echo -e "\n${YELLOW}── Summary${NC}"
if [[ ${#FAILED_CHECKS[@]} -eq 0 ]]; then
  echo -e "  ${GREEN}All checks passed.${NC}"
else
  echo -e "  ${RED}${#FAILED_CHECKS[@]} check(s) failed:${NC}"
  for c in "${FAILED_CHECKS[@]}"; do
    echo -e "    ${RED}✗${NC} ${c}"
  done
fi
printf '\nLogs in %s/\n\n' "${LOGS_DIR}"
