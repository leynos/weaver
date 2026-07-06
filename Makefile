.PHONY: help all clean test test-workflow-contracts build release lint fmt check-fmt markdownlint nixie typecheck install

TARGET ?= weaver
USER_CARGO := $(HOME)/.cargo/bin/cargo
USER_MDFORMAT := $(HOME)/.local/bin/mdformat-all
USER_MDLINT := $(HOME)/.bun/bin/markdownlint-cli2
USER_WHITAKER := $(HOME)/.local/bin/whitaker
USER_BIN_PATH := $(HOME)/.cargo/bin:$(HOME)/.local/bin:$(HOME)/.bun/bin
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(wildcard $(USER_CARGO)),cargo)
BUILD_JOBS ?=
RUST_FLAGS ?=
RUST_FLAGS := -D warnings $(RUST_FLAGS)
RUSTDOC_FLAGS ?=
RUSTDOC_FLAGS := -D warnings $(RUSTDOC_FLAGS)
CARGO_FLAGS ?= --workspace --all-targets --all-features
CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(RUST_FLAGS)
TEST_FLAGS ?= $(CARGO_FLAGS)
TEST_CMD := $(if $(shell $(CARGO) nextest --version 2>/dev/null),nextest run,test)
MDFORMAT ?= $(or $(shell command -v mdformat-all 2>/dev/null),$(wildcard $(USER_MDFORMAT)),mdformat-all)
MDLINT ?= $(or $(shell command -v markdownlint-cli2 2>/dev/null),$(wildcard $(USER_MDLINT)),markdownlint-cli2)
NIXIE ?= nixie
NIXIE_FLAGS ?= --no-sandbox
WHITAKER ?= $(or $(shell command -v whitaker 2>/dev/null),$(wildcard $(USER_WHITAKER)),whitaker)

build: target/debug/$(TARGET) ## Build debug binary
release: target/release/$(TARGET) ## Build release binary

all: check-fmt lint test ## Perform a comprehensive check of code

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) $(TEST_CMD) $(TEST_FLAGS) $(BUILD_JOBS)
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) test --doc --workspace --all-features

test-workflow-contracts: ## Validate the mutation-testing caller contract
	uv run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

target/%/$(TARGET): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(filter release,$*),--release) --bin $(TARGET)

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --no-deps --workspace
	$(CARGO) clippy $(CLIPPY_FLAGS)
	PATH="$(USER_BIN_PATH):$(PATH)" RUSTFLAGS="$(RUST_FLAGS)" $(WHITAKER) --all -- $(CARGO_FLAGS)

typecheck: ## Type-check without building
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) check $(CARGO_FLAGS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	PATH="$(USER_BIN_PATH):$(PATH)" $(MDFORMAT)

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files
	PATH="$(USER_BIN_PATH):$(PATH)" $(MDLINT) '**/*.md'

nixie: ## Validate Mermaid diagrams
	# Use `make nixie NIXIE_FLAGS=` to enable sandboxed mode locally.
	$(NIXIE) $(NIXIE_FLAGS)

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'

install: ## Install weaver and weaverd binaries
	$(CARGO) install --locked --path crates/weaver-cli
	$(CARGO) install --locked --path crates/weaverd
