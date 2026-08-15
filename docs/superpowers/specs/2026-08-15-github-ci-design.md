# Feedea — GitHub CI for Cross-Platform Builds

Date: 2026-08-15
Status: Approved design

Adds GitHub Actions workflows to build and test Feedea on every push/PR and to
produce release binaries for Linux (x64, arm64), Windows x64, and macOS (arm64,
x64) on version tags.

## 1. Goals & non-goals

### Goals
- CI on every push and PR: build frontend + backend, run lint/typecheck/tests.
- Release workflow on tags `v*`: build a release binary for each target on a
  native GitHub runner, run the end-to-end smoke test on Linux x64, and attach
  all binaries to the GitHub Release.
- Reproducible toolchain via a committed `rust-toolchain.toml` (stable).
- Preserve the existing single-binary artifact: `bun run build` then
  `cargo build --release` (build.rs requires `frontend/dist`).

### Non-goals (v1)
- Code signing (Windows/macOS), notarization, Homebrew cask, Docker images,
  SBOM, nightly builds, or cross-compilation from a single runner.
- Building Windows arm64 (no native GitHub arm64 Windows runner).

## 2. Toolchain

`rust-toolchain.toml` at repo root:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

Verified: the code compiles cleanly on stable rust 1.97.1 (edition 2024 is
stable; no `#![feature]` attributes anywhere). CI and local builds then match.

## 3. Workflow: CI on push/PR (`.github/workflows/ci.yml`)

Triggers: `push` (main) and `pull_request`.

Single job `test` on `ubuntu-latest`:
1. Checkout.
2. `oven-sh/setup-bun` (latest).
3. `dtolnay/rust-toolchain@stable` (reads `rust-toolchain.toml`).
4. Cache cargo via `Swatinem/rust-cache`.
5. `bun install` (working-directory `frontend`).
6. `bun run build` (working-directory `frontend`).
7. `cargo fmt --check`.
8. `cargo clippy --all-targets -- -D warnings`.
9. `cargo test --all-targets`.
10. `bun run typecheck` + `bun run lint` (working-directory `frontend`).

## 4. Workflow: Release on tags (`.github/workflows/release.yml`)

Triggers: `push` tags `v*`.

### Matrix (native runners)

| target            | runner               | artifact name             | smoke |
|-------------------|----------------------|---------------------------|-------|
| linux-x64         | `ubuntu-latest`      | `feedea-linux-x64`        | yes   |
| linux-arm64       | `ubuntu-24.04-arm`   | `feedea-linux-arm64`      | no    |
| windows-x64       | `windows-latest`     | `feedea-windows-x64.exe`  | no    |
| macos-arm64       | `macos-latest`       | `feedea-macos-arm64`      | no    |
| macos-x64         | `macos-13`           | `feedea-macos-x64`        | no    |

Each matrix job:
1. Checkout.
2. `oven-sh/setup-bun`.
3. `dtolnay/rust-toolchain@stable`.
4. `Swatinem/rust-cache`.
5. `bun install` + `bun run build` (frontend).
6. `cargo build --release`.
7. Rename the binary to the artifact name (Windows: `rssea.exe` is now
   `feedea.exe`; PowerShell for the copy/rename).
8. Run smoke test **on linux-x64 only**: `bash scripts/smoke.sh` against the
   built binary (the script auto-runs `make build` if the binary is missing,
   but here it exists — verify it uses the prebuilt binary).
9. Upload via `actions/upload-artifact`.

A final `release` job (`needs: build`) runs on `ubuntu-latest`:
- `softprops/action-gh-release` downloads all artifacts and attaches them to
  the tagged Release (draft: true so the user reviews before publishing).

### Smoke-test detail (linux-x64)
`scripts/smoke.sh` requires the release binary at `target/release/feedea`.
After `cargo build --release` it exists, so the script's auto-build branch is a
no-op. It uses `python3 -m http.server` for the fixture feed — present on
ubuntu-latest.

## 5. Naming & notes

- Binary name: `feedea` (cargo package `feedea`, renamed from `rssea`).
- Artifact names use `feedea-<os>-<arch>[.exe]`.
- Release is draft so the user can review before publish.
- GPL-3.0-or-later (news-flash) — already noted in the README; release notes
  can mention it.

## 6. Testing the workflows

- CI workflow can be exercised by pushing to a branch + opening a PR (once the
  repo is on GitHub).
- The release workflow runs only on tags; until the first tag, its YAML is
  validated with `actionlint` if available (or a YAML lint in CI).
- The Linux arm64 + macOS runners are public-preview hosted runners; if
  unavailable on the repo's plan, they fall back to a documented note.
