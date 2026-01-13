.PHONY: fmt
fmt:
	cargo fmt --quiet --all

.PHONY: check-fmt
check-fmt:
	cargo fmt --check --quiet --all

.PHONY: clippy
clippy:
	cargo clippy --workspace --locked --quiet -- -D warnings --no-deps

.PHONY: test
test:
	cargo test --workspace --release --locked --quiet --lib --bins

.PHONY: install
install:
	cargo install --path crates/sembly-cli --locked
	cargo install --path crates/forester --locked

.PHONY: deny
deny:
	cargo deny --no-default-features check licenses bans sources

.PHONY: lint-loc
lint-loc:
	python3 scripts/lint_loc.py

.PHONY: check
check: check-fmt clippy deny lint-loc test

.PHONY: integ
integ: check
	cargo test --workspace --locked --quiet -- --ignored

.PHONY: build
build:
	cargo build --workspace --locked --quiet

.PHONY: release-build
release-build:
	cargo build --workspace --release --locked --quiet

.PHONY: lint-style
lint-style:
	cargo run --quiet --release --package syn-lint

# Documentation
.PHONY: book book-serve
book:
	mdbook build crates/crumbly-cli/book

book-serve:
	mdbook serve crates/crumbly-cli/book
