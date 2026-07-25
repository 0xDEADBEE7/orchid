.PHONY: help build pack install-bin test lint check metrics clean

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

.DEFAULT_GOAL := help
