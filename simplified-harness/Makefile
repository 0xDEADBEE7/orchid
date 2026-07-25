.PHONY: help build install-bin test lint check metrics clean

help:
	@echo "Available targets:"
	@echo "  make build   - Build release binary"
	@echo "  make install-bin - Install as ../bin/simple-orchid"
	@echo "  make clean   - Remove build artifacts"
	@echo "  make test    - Run tests"
	@echo "  make lint    - Run clippy and fmt check"
	@echo "  make check   - lint + test"
	@echo "  make metrics - Run LoC, file-size, and assay metrics"

build:
	cargo build --release

install-bin: build
	mkdir -p ../bin
	cp target/release/orchid-simplified ../bin/simple-orchid

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
