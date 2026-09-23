.PHONY: help all clean test dev-test test-workflow-contracts build dev-build release lint fmt \
	check-fmt markdownlint nixie typecheck install spelling

TARGET ?= weaver
USER_CARGO := $(HOME)/.cargo/bin/cargo
USER_MDLINT := $(HOME)/.bun/bin/markdownlint-cli2
USER_WHITAKER := $(HOME)/.local/bin/whitaker
USER_BIN_PATH := $(HOME)/.cargo/bin:$(HOME)/.local/bin:$(HOME)/.bun/bin
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(wildcard $(USER_CARGO)),cargo)
# The pinned Cranelift component is available for Linux and macOS hosts, but
# not FreeBSD. Release and coverage retain Cargo's ordinary LLVM backend.
HOST_OS ?= $(shell uname -s)
DEV_FAST_CONFIG := $(if $(filter Linux Darwin,$(HOST_OS)),--config tools/dev-fast/config.toml)
# RUSTFLAGS overrides Cargo's target rustflags. CI's setup-rust action exports
# it, so preserve its value and add mold explicitly for every Linux debug gate.
DEV_FAST_RUST_FLAGS := $(if $(filter Linux,$(HOST_OS)),-Clink-arg=-fuse-ld=mold)
DEV_FAST_LINKER_ENV = $(if $(DEV_FAST_RUST_FLAGS),RUSTFLAGS="$(strip $(RUSTFLAGS) $(DEV_FAST_RUST_FLAGS))")
BUILD_JOBS ?=
RUST_FLAGS ?=
override RUST_FLAGS := $(strip -D warnings $(RUST_FLAGS))
RUSTDOC_FLAGS ?=
override RUSTDOC_FLAGS := $(strip -D warnings $(RUSTDOC_FLAGS))
CARGO_FLAGS ?= --workspace --all-targets --all-features
CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(RUST_FLAGS)
TEST_FLAGS ?= $(CARGO_FLAGS)
TEST_CMD := $(if $(shell $(CARGO) nextest --version 2>/dev/null),nextest run,test)
MDLINT ?= $(shell command -v markdownlint-cli2 2>/dev/null || printf '%s' "$$HOME/.bun/bin/markdownlint-cli2")
# `make fmt` and `make check-fmt` call mdtablefix directly. `--git` selects the
# Markdown files Git tracks and `--include-untracked` adds the untracked files
# Git does not ignore, so a new document is formatted before it is staged.
# Both modes need mdtablefix 0.6.0 or later; CI pins the version at the
# install-mdtablefix step.
MDTABLEFIX ?= mdtablefix
MDTABLEFIX_SELECT = --git --include-untracked
MDTABLEFIX_RULES = --wrap --renumber --breaks --ellipsis --fences
NIXIE ?= nixie
NIXIE_FLAGS ?= --no-sandbox
WHITAKER ?= $(or $(shell command -v whitaker 2>/dev/null),$(wildcard $(USER_WHITAKER)),whitaker)
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.1
# CI pins uv to Python 3.11, so ask for the interpreter the builder requires.
TYPOS_CONFIG_BUILDER = $(UV_ENV) $(UV) tool run --python 3.14 --from \
	"git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder

build: ## Build debug binary
	$(DEV_FAST_LINKER_ENV) $(CARGO) $(DEV_FAST_CONFIG) build $(BUILD_JOBS) --bin $(TARGET)

dev-build: build ## Build with the supported development backend

release: ## Build release binary
	$(CARGO) build $(BUILD_JOBS) --release --bin $(TARGET)

all: ## Perform a comprehensive check of code and prose
	$(MAKE) check-fmt
	$(MAKE) lint
	$(MAKE) test
	$(MAKE) spelling

clean: ## Remove build artefacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="$(strip $(RUSTFLAGS) $(RUST_FLAGS) $(DEV_FAST_RUST_FLAGS))" $(CARGO) $(DEV_FAST_CONFIG) $(TEST_CMD) $(TEST_FLAGS) $(BUILD_JOBS)
	RUSTFLAGS="$(strip $(RUSTFLAGS) $(RUST_FLAGS) $(DEV_FAST_RUST_FLAGS))" $(CARGO) $(DEV_FAST_CONFIG) test --doc --workspace --all-features

dev-test: test ## Test with the supported development backend

test-workflow-contracts: ## Validate workflow caller and runner-placement contracts
	uv run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(DEV_FAST_LINKER_ENV) $(CARGO) $(DEV_FAST_CONFIG) doc --no-deps --workspace
	$(DEV_FAST_LINKER_ENV) $(CARGO) $(DEV_FAST_CONFIG) clippy $(CLIPPY_FLAGS)
	PATH="$(USER_BIN_PATH):$(PATH)" RUSTFLAGS="$(RUST_FLAGS)" $(WHITAKER) --all -- $(CARGO_FLAGS)

typecheck: ## Type-check without building
	RUSTFLAGS="$(strip $(RUSTFLAGS) $(RUST_FLAGS) $(DEV_FAST_RUST_FLAGS))" $(CARGO) $(DEV_FAST_CONFIG) check $(CARGO_FLAGS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)
	@unset FORCE_COLOR; $(MDLINT) --fix "**/*.md"

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check
	$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)

markdownlint: spelling ## Lint Markdown files and enforce spelling
	PATH="$(USER_BIN_PATH):$(PATH)" $(MDLINT) '**/*.md'

spelling: ## Enforce en-GB-oxendict spelling in Markdown prose
	$(TYPOS_CONFIG_BUILDER) gate --repository .

nixie: ## Validate Mermaid diagrams
	# Use `make nixie NIXIE_FLAGS=` to enable sandboxed mode locally.
	$(NIXIE) $(NIXIE_FLAGS)

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'

install: ## Install weaver and weaverd binaries
	$(CARGO) install --locked --path crates/weaver-cli
	$(CARGO) install --locked --path crates/weaverd
