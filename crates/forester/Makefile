.PHONY: fmt
fmt:
	cargo fmt --check --quiet

.PHONY: clippy
clippy:
	cargo clippy --locked --quiet -- -D warnings --no-deps

.PHONY: test
test:
	cargo test --release --locked --quiet

.PHONY: deny
deny:
	cargo deny --no-default-features check licenses bans sources

.PHONY: check
check: fmt clippy deny test

.PHONY: integ
integ: check
	cargo test --locked --quiet -- --ignored

.PHONY: build
build:
	cargo build --locked --quiet

.PHONY: release-build
release-build:
	cargo build --release --locked --quiet
