.PHONY: dev integrate test

dev:
	rustup component add rustfmt

integrate:
	./integrate/check

test:
	cargo fmt -- --check
	cargo test --locked
