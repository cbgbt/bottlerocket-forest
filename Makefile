.PHONY: fmt
fmt:
	cargo fmt --check --quiet --all

.PHONY: clippy
clippy:
	cargo clippy --workspace --locked --quiet -- -D warnings --no-deps

.PHONY: test
test:
	cargo test --workspace --release --locked --quiet --lib --bins

.PHONY: deny
deny:
	cargo deny --no-default-features check licenses bans sources

.PHONY: check
check: fmt clippy deny test

.PHONY: integ
integ: check
	cargo test --workspace --locked --quiet -- --ignored

.PHONY: build
build:
	cargo build --workspace --locked --quiet

.PHONY: release-build
release-build:
	cargo build --workspace --release --locked --quiet
