.PHONY: dev setup build release install install-release install-sync sync-init sync-status sync-now test lint check fmt clean help

DEV_HOME := $(HOME)/.clings-dev

# ── Lancer ────────────────────────────────────
dev:
	@echo "▶ Lancement cargo run watch (DB $(DEV_HOME))"
	CLINGS_HOME=$(DEV_HOME) cargo run -- watch

dev-%:
	@echo "▶ Lancement cargo run $* (DB $(DEV_HOME))"
	CLINGS_HOME=$(DEV_HOME) cargo run -- $*

# ── Build & Install ──────────────────────────
build:
	@echo "▶ Build debug"
	cargo build

release:
	@echo "▶ Build release optimisé"
	cargo build --release

setup: build
	@echo "▶ Création symlink clings-dev"
	mkdir -p ~/.local/bin
	ln -sf $(CURDIR)/target/debug/clings ~/.local/bin/clings-dev
	@echo "  clings-dev → $(CURDIR)/target/debug/clings"

install-release:
	@echo "▶ cargo install (release)"
	cargo install --path .

install-sync:
	@echo "▶ Installation clings-sync"
	install -Dm755 scripts/clings-sync ~/.local/bin/clings-sync
	@echo "  clings-sync → ~/.local/bin/clings-sync"

install: install-release install-sync

# ── Sync Git ─────────────────────────────────
sync-init:
ifndef REMOTE
	$(error REMOTE est requis — ex: make sync-init REMOTE=git@github.com:user/clings-sync.git)
endif
	@echo "▶ Initialisation sync Git → $(REMOTE)"
	CLINGS_HOME=$(DEV_HOME) cargo run -- sync init $(REMOTE)

sync-status:
	@echo "▶ État du sync Git"
	CLINGS_HOME=$(DEV_HOME) cargo run -- sync status

sync-now:
	@echo "▶ Sync Git (pull + push)"
	CLINGS_HOME=$(DEV_HOME) cargo run -- sync now

# ── Qualité ───────────────────────────────────
test:
	@echo "▶ Tests unitaires"
	cargo test

lint:
	@echo "▶ Lint (clippy -D warnings)"
	cargo clippy -- -D warnings

check: test lint

fmt:
	@echo "▶ Formatage (rustfmt)"
	cargo fmt

clean:
	@echo "▶ Nettoyage target/"
	cargo clean

# ── Help ──────────────────────────────────────
help:
	@echo "Cibles disponibles:"
	@echo ""
	@echo "  Lancer:"
	@echo "    dev           — cargo run watch (DB ~/.clings-dev)"
	@echo "    dev-<cmd>     — cargo run <cmd> (DB ~/.clings-dev)"
	@echo ""
	@echo "  Build & Install:"
	@echo "    build         — cargo build (debug)"
	@echo "    release       — cargo build --release"
	@echo "    setup         — build debug + symlink clings-dev"
	@echo "    install       — cargo install (release) + install-sync"
	@echo "    install-sync  — installe clings-sync dans ~/.local/bin/"
	@echo ""
	@echo "  Sync Git:"
	@echo "    sync-init     — initialiser le sync (REMOTE=git@...)"
	@echo "    sync-status   — afficher l'état du sync"
	@echo "    sync-now      — forcer pull + push"
	@echo ""
	@echo "  Qualité:"
	@echo "    test          — cargo test"
	@echo "    lint          — cargo clippy"
	@echo "    check         — test + lint"
	@echo "    fmt           — cargo fmt"
	@echo "    clean         — cargo clean"

.DEFAULT_GOAL := help
