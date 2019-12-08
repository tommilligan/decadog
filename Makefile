.PHONY: dev integrate test

dev:
	rustup component add rustfmt clippy

test:
	cargo fmt --all -- --check
	cargo clippy --all --all-targets --all-features -- -D warnings
	cargo test --all --locked
	cargo test --all --no-default-features --locked
