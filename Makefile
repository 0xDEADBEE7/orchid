.PHONY: build clean test lint check metrics help

help:
	@echo "Available targets:"
	@echo "  make build   - Build release binary"
	@echo "  make clean   - Remove build artifacts"
	@echo "  make test    - Run tests (single-threaded)"
	@echo "  make lint    - Run clippy and fmt check"
	@echo "  make check   - lint + test"
	@echo "  make metrics - Run code health metrics (LoC, complexity, binary size)"
	@echo "  make help    - Show this help"

build:
	./.scripts/build.sh

clean:
	./.scripts/clean.sh

test:
	./.scripts/test.sh

lint:
	./.scripts/lint.sh

check: lint test

metrics:
	./.scripts/metrics.sh

.DEFAULT_GOAL := help
