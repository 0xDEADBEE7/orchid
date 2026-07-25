#!/usr/bin/env bash
set -euo pipefail

BOLD='\033[1m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; RESET='\033[0m'
WARN_LOC=300; RED_LOC=400
section() { echo -e "\n${BOLD}${CYAN}=== $1 ===${RESET}"; }
warn() { echo -e "${YELLOW}  ⚠  $1${RESET}"; }
error() { echo -e "${RED}  ✖  $1${RESET}"; }
ok() { echo -e "${GREEN}  ✔  $1${RESET}"; }

section "Lines of Code (cloc)"
if command -v cloc >/dev/null 2>&1; then
  cloc src --quiet
else
  warn "cloc not found — install with: brew install cloc"
fi

section "File Size Warnings"
red=0; yellow=0
while IFS= read -r file; do
  loc=$(wc -l < "$file" | tr -d ' ')
  if [ "$loc" -ge "$RED_LOC" ]; then error "$file → $loc lines"; red=$((red + 1));
  elif [ "$loc" -ge "$WARN_LOC" ]; then warn "$file → $loc lines"; yellow=$((yellow + 1)); fi
done < <(find src -name '*.rs' -print | sort)
[ "$red" -eq 0 ] && [ "$yellow" -eq 0 ] && ok "All source files under ${WARN_LOC} lines"
[ "$yellow" -gt 0 ] && warn "$yellow source file(s) in yellow zone"
[ "$red" -gt 0 ] && error "$red source file(s) in red zone — refactor before adding code"

section "Complexity & Maintainability (assay)"
if command -v assay >/dev/null 2>&1; then
  assay $(find src -name '*.rs' -print | sort)
else
  warn "assay not found — skipping complexity metrics"
fi

section "Binary Size"
if [ -f target/release/orchid ]; then
  bytes=$(wc -c < target/release/orchid | tr -d ' ')
  ok "target/release/orchid → $(ls -lh target/release/orchid | awk '{print $5}') (${bytes} bytes)"
else
  warn "Release binary not found — run make build first"
fi

section "Summary"
[ "$red" -eq 0 ] && ok "No red-zone source files"
echo "Production Rust LOC: $(find src -name '*.rs' -print0 | xargs -0 cat | wc -l | tr -d ' ')"
