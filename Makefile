.PHONY: help all clean test test-workflow-contracts build release lint fmt \
	check-fmt markdownlint nixie typos typecheck install spelling \
	spelling-helper-test
.PHONY: spelling spelling-config spelling-phrase-check spelling-typos-check typos

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
UV ?= uv
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
RUFF_VERSION ?= 0.15.12
TYPOS_VERSION ?= 1.48.0
SPELLING_PY_SRCS := scripts/generate_typos_config.py scripts/typos_rollout_check.py scripts/typos_rollout.py \
	scripts/typos_rollout_cache.py scripts/typos_rollout_http.py scripts/typos_rollout_policy.py \
	scripts/tests/conftest.py scripts/tests/test_typos_rollout.py scripts/tests/test_typos_rollout_check.py \
	scripts/tests/test_typos_rollout_policy.py \
	scripts/tests/test_typos_rollout_semantics.py \
	scripts/tests/test_typos_rollout_contract.py \
	scripts/tests/test_typos_rollout_refresh.py \
	scripts/tests/typos_rollout_test_support.py

build: target/debug/$(TARGET) ## Build debug binary
release: target/release/$(TARGET) ## Build release binary

all: check-fmt lint test spelling ## Perform a comprehensive check of code and prose

clean: ## Remove build artefacts
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

markdownlint: spelling ## Lint Markdown files and enforce spelling
	PATH="$(USER_BIN_PATH):$(PATH)" $(MDLINT) '**/*.md'

spelling: spelling-typos-check ## Enforce en-GB-oxendict spelling in Markdown prose

spelling-config: spelling-helper-test
	@$(UV_ENV) $(UV) run scripts/generate_typos_config.py
	@git ls-files --error-unmatch typos.toml >/dev/null
	@git diff --exit-code -- typos.toml

spelling-phrase-check: spelling-config
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python 3.13 scripts/typos_rollout_check.py --repository .

spelling-typos-check: spelling-phrase-check
	@git ls-files -z '*.md' | xargs -0 -r env $(UV_ENV) \
		$(UV) tool run typos@$(TYPOS_VERSION) --config typos.toml --force-exclude

spelling-helper-test: ## Validate the shared spelling-policy integration
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --isolated --target-version py313 --check $(SPELLING_PY_SRCS)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --isolated --target-version py313 $(SPELLING_PY_SRCS)
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python 3.13 --with pytest==9.0.2 --with pytest-cov==7.0.0 \
		python -m pytest scripts/tests -c /dev/null --rootdir=. -p no:cacheprovider \
		--cov=generate_typos_config --cov=typos_rollout_check --cov=typos_rollout --cov=typos_rollout_cache --cov=typos_rollout_http --cov-fail-under=90

nixie: ## Validate Mermaid diagrams
	# Use `make nixie NIXIE_FLAGS=` to enable sandboxed mode locally.
	$(NIXIE) $(NIXIE_FLAGS)

typos: spelling ## Compatibility alias for the complete spelling gate

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'

install: ## Install weaver and weaverd binaries
	$(CARGO) install --locked --path crates/weaver-cli
	$(CARGO) install --locked --path crates/weaverd
