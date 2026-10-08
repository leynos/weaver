.PHONY: help all clean test test-workflow-contracts build release lint fmt \
	check-fmt markdownlint nixie typecheck install spelling

TARGET ?= weaver
USER_CARGO := $(HOME)/.cargo/bin/cargo
USER_MDLINT := $(HOME)/.bun/bin/markdownlint-cli2
USER_WHITAKER := $(HOME)/.local/bin/whitaker
USER_BIN_PATH := $(HOME)/.cargo/bin:$(HOME)/.local/bin:$(HOME)/.bun/bin
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(wildcard $(USER_CARGO)),cargo)
BUILD_JOBS ?=
RUST_FLAGS ?=
RUST_FLAGS := -D warnings $(RUST_FLAGS)
# The build standard: every `rustflags` source in `.cargo/config.toml` carries
# the parallel frontend, and the Linux source adds mold. Assigning `RUSTFLAGS`
# replaces those sources outright, so the gate targets restate the flags here.
# The gate targets add them to any inherited `RUSTFLAGS` (setup-rust exports
# one in CI) instead of replacing it.
# `make release` adds neither standard flag (a caller's own flags pass through),
# and coverage takes them only when its caller leaves `RUSTFLAGS` unset.
STANDARD_THREADS_FLAG ?= -Zthreads=8
STANDARD_MOLD_FLAG ?= -Clink-arg=-fuse-ld=mold
BUILD_HOST_OS := $(shell uname -s)
# mold is added only when the machine doing the build is Linux (only Make can
# tell whether it has mold) and every compilation target of that command is
# Linux too. Each recipe line reads its own targets from the arguments it passes:
# each explicit `--target X` or `--target=X` before any standalone `--` (at the
# start, the middle or the end) is one target (Cargo builds for all of them),
# and with none the command takes `CARGO_BUILD_TARGET`, which names the host
# when unset.
SPACE := $(subst ,, )
standard_args = $(filter-out __ARG0__ __ARG1__,$(subst __SP__, ,$(firstword $(subst __SP__--__SP__, ,$(subst $(SPACE),__SP__,__ARG0__ $(strip $(1)) __ARG1__)))))
standard_targets = $(patsubst --target=%,%,$(filter --target=%,$(subst --target ,--target=,$(call standard_args,$(1)))))
standard_non_linux = $(foreach target,$(1),$(if $(or $(findstring -linux-,$(target)),$(filter host-tuple,$(target))),,$(target)))
standard_env_is_linux = $(if $(CARGO_BUILD_TARGET),$(or $(findstring -linux-,$(CARGO_BUILD_TARGET)),$(filter host-tuple,$(CARGO_BUILD_TARGET))),yes)
standard_is_linux = $(if $(call standard_targets,$(1)),$(if $(strip $(call standard_non_linux,$(call standard_targets,$(1)))),,yes),$(standard_env_is_linux))
standard_rustflags = $(STANDARD_THREADS_FLAG)$(if $(filter Linux,$(BUILD_HOST_OS)),$(if $(call standard_is_linux,$(1)), $(STANDARD_MOLD_FLAG)))
# Release builds add neither standard flag: assigning `RUSTFLAGS`, even to an
# empty inherited value, displaces every `rustflags` source in the
# configuration, and the caller's own value passes through untouched.
RELEASE_RUSTFLAGS = RUSTFLAGS="$${RUSTFLAGS-}"
# Debug builds keep a caller's exported flags and add the standard ones,
# since an inherited `RUSTFLAGS` would otherwise displace the configuration.
DEBUG_RUSTFLAGS = RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(call standard_rustflags,)"
RUSTDOC_FLAGS ?=
RUSTDOC_FLAGS := -D warnings $(RUSTDOC_FLAGS)
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

# The CV-005 CodeScene contracts live in shared-actions and run from a full
# commit, so a fix is a pin bump. `.github/cv005.toml` holds this repository's
# only parameters.
CV005_CONTRACTS_REF ?= 88977798a5c3bae1549afb99642529488c665276
CV005_CONTRACTS = $(UV_ENV) $(UV) tool run --python 3.13 \
	--from 'git+https://github.com/leynos/shared-actions@$(CV005_CONTRACTS_REF)\#subdirectory=packages/cv005-contracts' \
	cv005-contracts

TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.3
# CI pins uv to Python 3.11, so ask for the interpreter the builder requires.
TYPOS_CONFIG_BUILDER = $(UV_ENV) $(UV) tool run --python 3.14 --from \
	"git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder

build: target/debug/$(TARGET) ## Build debug binary
release: target/release/$(TARGET) ## Build release binary

all: check-fmt lint test spelling test-workflow-contracts ## Perform a comprehensive check of code and prose

clean: ## Remove build artefacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(RUST_FLAGS) $(call standard_rustflags,$(TEST_FLAGS))" $(CARGO) $(TEST_CMD) $(TEST_FLAGS) $(BUILD_JOBS)
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(RUST_FLAGS) $(call standard_rustflags,)" $(CARGO) test --doc --workspace --all-features

test-workflow-contracts: ## Validate workflow caller and runner-placement contracts
	$(CV005_CONTRACTS) check --repository .
	uv run --with 'pytest>=8' --with 'pyyaml>=6' --with 'hypothesis>=6' pytest tests/workflow_contracts -q

target/%/$(TARGET): ## Build binary in debug or release mode
	$(if $(filter release,$*),$(RELEASE_RUSTFLAGS),$(DEBUG_RUSTFLAGS)) $(CARGO) build $(BUILD_JOBS) $(if $(filter release,$*),--release) --bin $(TARGET)

lint: ## Run Clippy with warnings denied
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(RUST_FLAGS) $(call standard_rustflags,)" RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --no-deps --workspace
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(RUST_FLAGS) $(call standard_rustflags,$(CLIPPY_FLAGS))" $(CARGO) clippy $(CLIPPY_FLAGS)
	PATH="$(USER_BIN_PATH):$(PATH)" RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(RUST_FLAGS) $(call standard_rustflags,$(CARGO_FLAGS))" $(WHITAKER) --all -- $(CARGO_FLAGS)

typecheck: ## Type-check without building
	RUSTFLAGS="$${RUSTFLAGS:+$$RUSTFLAGS }$(RUST_FLAGS) $(call standard_rustflags,$(CARGO_FLAGS))" $(CARGO) check $(CARGO_FLAGS)

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
