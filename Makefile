# ABOUTME: The single entry point for building, testing, linting, and documenting Cupel.
# ABOUTME: Humans, agents, and CI call these targets; nothing calls cargo directly for these tasks.

CARGO ?= cargo

# A fingerprint of the working tree: status (including untracked files) plus the content diff.
TREE_STATE = { git status --porcelain --untracked-files=all; git diff; } | git hash-object --stdin
EMPTY_TREE_STATE = printf '' | git hash-object --stdin
TREE_STATE_FILE = target/tree-state-before-check

.PHONY: build test lint fmt doc matrix readme check clean-check dogfood install

build:
	$(CARGO) build --workspace

# UPDATE_GOLDENS=1 make test rewrites golden envelopes instead of comparing them (ADR-003).
test:
	$(CARGO) test --workspace

lint:
	$(CARGO) fmt --all --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings

fmt:
	$(CARGO) fmt --all

doc:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --no-deps

# The current checks × languages support table, derived from the registries (design §8.3.5).
matrix:
	@$(CARGO) run --quiet --package cupel-cli -- describe matrix --format text

# Rewrites the README's generated sections (exit codes, check table, support matrix) from
# `cupel describe`. `make test` fails while they are stale (ADR-073).
readme:
	UPDATE_README=1 $(CARGO) test --package cupel-cli --test readme

# The pre-commit bar. The tree is fingerprinted before the build so clean-check can
# verify the build itself wrote nothing outside .gitignore (see ADR-010).
check:
	@mkdir -p target && $(TREE_STATE) > $(TREE_STATE_FILE)
	@$(MAKE) --no-print-directory lint test doc
	@$(MAKE) --no-print-directory clean-check EXPECTED_TREE_STATE="$$(cat $(TREE_STATE_FILE))"

# Fails when the working tree differs from EXPECTED_TREE_STATE; with no expectation
# (as in CI on a fresh checkout) the tree must be completely clean.
clean-check:
	@expected="$(EXPECTED_TREE_STATE)"; \
	[ -n "$$expected" ] || expected="$$($(EMPTY_TREE_STATE))"; \
	actual="$$($(TREE_STATE))"; \
	if [ "$$actual" != "$$expected" ]; then \
		echo "clean-check: the working tree changed during the build:" >&2; \
		git status --porcelain --untracked-files=all >&2; \
		exit 1; \
	fi

# Cupel grades itself with its own cupel.toml (plan §2.3, P2.10). The default is the changed
# gate: everything that differs from the merge base with `main`. On `main` itself there is
# nothing to diff against, so CI runs the tree gate there with `make dogfood DOGFOOD_SCOPE=tree`.
# `--changed` needs a local `main` branch and the history back to the merge base.
DOGFOOD_SCOPE ?= changed

# `tests.flaky` and `tests.mutation` are skipped: the flake engine reruns Cupel's whole suite three
# times, one test at a time, which takes tens of minutes (P3.5 Notes), and every mutant reruns it
# once more after rebuilding (P3.4 Notes); `make check` runs that suite on every commit.
# `--no-install`: grading Cupel never asks to install a tool, even from a terminal (ADR-081).
dogfood:
	$(CARGO) run --quiet --package cupel-cli -- check $(if $(filter tree,$(DOGFOOD_SCOPE)),,--changed) --gate --no-install --skip tests.flaky --skip tests.mutation --format text

install:
	$(CARGO) install --path crates/cupel-cli
