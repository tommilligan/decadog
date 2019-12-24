.PHONY: dev test doc

dev:
	rustup component add rustfmt clippy

test:
	cargo fmt --all -- --check
	cargo clippy --all --all-targets --all-features -- -D warnings
	cargo test --all --locked
	cargo test --all --doc --locked

doc:
	cargo doc --workspace --no-deps --locked
