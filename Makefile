.PHONY: dev build run test clean frontend-dev backend-dev install-watch

DATA_DIR ?= $(HOME)/.local/share/rssea
BACKEND_PORT ?= 3000
FRONTEND_PORT ?= 5173

dev: backend-dev frontend-dev

backend-dev:
	cargo watch -x "run -- --data-dir $(DATA_DIR) --port $(BACKEND_PORT)"

frontend-dev:
	@echo "Frontend dev server (bun) is added in Phase 4. Run: cd frontend && bun run dev"

build:
	cd frontend && bun run build
	cargo build --release

run:
	cargo run --release -- --data-dir $(DATA_DIR)

test:
	cargo test
	@if [ -f frontend/package.json ]; then cd frontend && bun run typecheck && bun run lint; else echo "frontend not present; skipping frontend checks"; fi

clean:
	cargo clean
	rm -rf frontend/dist frontend/node_modules

install-watch:
	cargo install cargo-watch
