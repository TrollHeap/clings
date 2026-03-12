.PHONY: build release dev install-release install-sync install dev-link watch list progress review stats test lint fmt clean help

CARGO_RUN = cargo run --

build:
	cargo build

release:
	cargo build --release

dev: build dev-link

install-release:
	cargo build --release
	cargo install --path .

install-sync:
	install -Dm755 scripts/clings-sync ~/.local/bin/clings-sync
	@echo "clings-sync → ~/.local/bin/clings-sync"

install: install-release install-sync

dev-link:
	mkdir -p ~/.local/bin
	ln -sf $(CURDIR)/target/debug/clings ~/.local/bin/clings-dev
	@echo "clings-dev → $(CURDIR)/target/debug/clings"

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
	@echo "  build           — cargo build (debug)"
	@echo "  release         — cargo build --release"
	@echo "  dev             — build debug + symlink clings-dev"
	@echo "  install-release — build release + installe dans ~/.cargo/bin/"
	@echo "  install-sync    — installe scripts/clings-sync dans ~/.local/bin/"
	@echo "  install         — install-release + install-sync"
	@echo "  dev-link        — symlink clings-dev → target/debug/clings"
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
