.PHONY: build release install watch list progress review stats test lint fmt clean help

CARGO_RUN = cargo run --

build:
	cargo build

release:
	cargo build --release

install:
	cargo install --path .

# Sous-commandes kf
watch:
	$(CARGO_RUN) watch

list:
	$(CARGO_RUN) list

progress:
	$(CARGO_RUN) progress

review:
	$(CARGO_RUN) review

stats:
	$(CARGO_RUN) stats

# Qualité
test:
	cargo test

lint:
	cargo clippy -- -D warnings

fmt:
	cargo fmt

clean:
	cargo clean

help:
	@echo "Cibles disponibles:"
	@echo "  build     — cargo build (debug)"
	@echo "  release   — cargo build --release"
	@echo "  install   — installe kf dans ~/.cargo/bin/"
	@echo "  watch     — kf watch (mode exercice par défaut)"
	@echo "  list      — kf list"
	@echo "  progress  — kf progress"
	@echo "  review    — kf review"
	@echo "  stats     — kf stats"
	@echo "  test      — cargo test"
	@echo "  lint      — cargo clippy"
	@echo "  fmt       — cargo fmt"
	@echo "  clean     — cargo clean"

.DEFAULT_GOAL := help
