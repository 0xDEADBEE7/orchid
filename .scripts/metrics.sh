#!/usr/bin/env bash
# Code health metrics: LoC (cloc), complexity (assay), file-size warnings, binary size.
set -euo pipefail

BOLD='\033[1m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
RESET='\033[0m'

WARN_LOC=200
RED_LOC=300

section() { echo -e "\n${BOLD}${CYAN}=== $1 ===${RESET}"; }
warn()    { echo -e "${YELLOW}  ⚠  $1${RESET}"; }
error()   { echo -e "${RED}  ✖  $1${RESET}"; }
ok()      { echo -e "${GREEN}  ✔  $1${RESET}"; }

# ---------------------------------------------------------------------------
section "Lines of Code (cloc)"
if ! command -v cloc &>/dev/null; then
  warn "cloc not found — install with: brew install cloc"
else
  echo -e "\n${BOLD}Production (src/)${RESET}"
  cloc src/ --quiet

  echo -e "\n${BOLD}Tests (tests/ + in-module test files)${RESET}"
  cloc tests/ --quiet
  # Count in-module test files (src/**/tests.rs)
  cloc $(find src -name 'tests.rs') --quiet 2>/dev/null || true
fi

# ---------------------------------------------------------------------------
section "File Size Warnings (per-file LoC)"
echo ""
FILES_YELLOW=0
FILES_RED=0

while IFS= read -r file; do
  loc=$(grep -c '' "$file" 2>/dev/null || true)
  if [ "$loc" -ge "$RED_LOC" ]; then
    error "$file  →  ${loc} lines  [RED: ≥${RED_LOC}]"
    FILES_RED=$((FILES_RED + 1))
  elif [ "$loc" -ge "$WARN_LOC" ]; then
    warn "$file  →  ${loc} lines  [YELLOW: ≥${WARN_LOC}]"
    FILES_YELLOW=$((FILES_YELLOW + 1))
  fi
done < <(find src tests -name '*.rs' | sort)

[ "$FILES_RED" -eq 0 ] && [ "$FILES_YELLOW" -eq 0 ] && ok "All files under ${WARN_LOC} lines"
[ "$FILES_YELLOW" -gt 0 ] && warn "${FILES_YELLOW} file(s) in yellow zone (${WARN_LOC}–$((RED_LOC - 1)) lines)"
[ "$FILES_RED" -gt 0 ] && error "${FILES_RED} file(s) in red zone (≥${RED_LOC} lines) — refactor before adding code"

# ---------------------------------------------------------------------------
section "Complexity & Maintainability (assay)"
if ! command -v assay &>/dev/null; then
  warn "assay not found — skipping complexity metrics"
else
  assay src/**/*.rs
fi

# ---------------------------------------------------------------------------
section "Binary Size"
if [ -f "target/release/stash" ]; then
  size_bytes=$(wc -c < target/release/stash)
  size_human=$(ls -lh target/release/stash | awk '{print $5}')
  ok "target/release/stash  →  ${size_human} (${size_bytes} bytes)"
  echo ""
  echo -e "  ${CYAN}Tip: run \`cargo build --release\` with RUSTFLAGS='-C strip=symbols' for a smaller binary.${RESET}"
else
  warn "Release binary not found — run \`make build\` first"
fi

# ---------------------------------------------------------------------------
section "Summary"
if command -v cloc &>/dev/null && command -v assay &>/dev/null; then
  ok "All metric tools available"
elif ! command -v assay &>/dev/null; then
  warn "assay not found — complexity metrics unavailable"
fi
[ "$FILES_RED" -gt 0 ] && error "${FILES_RED} red file(s) require refactoring before new code is added"
echo ""
