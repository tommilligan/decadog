.PHONY: dev integrate test

dev:
	rustup component add rustfmt clippy

test:
	cargo fmt -- --check
	cargo clippy --all-targets --all-features -- -D 'clippy::all'
	cargo test --all --locked
	cargo test --all --all-features --locked
