.PHONY: help build pack install-bin test lint check metrics clean prompts

RELEASE_BINARY := target/release/orchid
STAGED_BINARY := bin/orchid

help:
	@echo "Available targets:"
	@echo "  make build   - Build and stage the uncompressed release binary"
	@echo "  make pack    - UPX-compress the staged release binary"
	@echo "  make install-bin - Build and stage as ./bin/orchid"
	@echo "  make clean   - Remove build artifacts"
	@echo "  make test    - Run tests"
	@echo "  make lint    - Run clippy and fmt check"
	@echo "  make check   - lint + test"
	@echo "  make metrics - Run LoC, file-size, and assay metrics"
	@echo "  make prompts - Regenerate the orchestrator prompt from docs/orchestrator/"

build:
	cargo build --release
	mkdir -p bin
	cp $(RELEASE_BINARY) $(STAGED_BINARY)

pack: build
	upx --best --lzma --force-macos $(STAGED_BINARY)

install-bin: build

clean:
	cargo clean

test:
	cargo test --offline

lint:
	cargo fmt --check
	cargo clippy --offline --all-targets -- -D warnings

check: lint test

metrics:
	./.scripts/metrics.sh

prompts:
	@{ printf '%s\n\n' '<!-- Generated from docs/orchestrator/*.md. Do not edit directly. -->'; \
	  sep=; for f in docs/orchestrator/*.md; do printf "$$sep"; cat "$$f"; sep='\n\n---\n\n'; done; \
	  printf '\n'; } > docs/resources/config/prompts/orchestrator.md

.DEFAULT_GOAL := help
