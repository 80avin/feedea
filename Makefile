.PHONY: dev build run test clean frontend-dev backend-dev

DATA_DIR ?= $(HOME)/.local/share/rssea
BACKEND_PORT ?= 3000
FRONTEND_PORT ?= 5173

dev:
	@$(MAKE) -j2 backend-dev frontend-dev

backend-dev:
	cargo run -- --data-dir $(DATA_DIR) --port $(BACKEND_PORT)

frontend-dev:
	cd frontend && bun run dev --port $(FRONTEND_PORT)

build:
	cd frontend && bun run build && cd ..
	cargo build --release

run:
	cargo run --release -- --data-dir $(DATA_DIR)

test:
	cd frontend && bun run build && cd ..
	cargo test
	@if [ -f frontend/package.json ]; then cd frontend && bun run typecheck && bun run lint; else echo "frontend not present; skipping frontend checks"; fi

clean:
	cargo clean
	rm -rf frontend/dist frontend/node_modules
