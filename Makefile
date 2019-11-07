.PHONY: dev integrate test

dev:
	rustup component add rustfmt

test:
	cargo fmt -- --check
	cargo test --locked
