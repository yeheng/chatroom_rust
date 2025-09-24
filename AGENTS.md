# Repository Guidelines

## Project Structure & Module Organization

- `crates/domain/`: Core domain models (entities, errors, repositories).
- `crates/application/`: Use cases, CQRS (commands/queries/handlers), application services.
- `crates/infrastructure/`: DB (sqlx), Kafka, Redis, websocket adapters.
- `crates/web-api/`: Axum-based HTTP/WebSocket API.
- `crates/tests/`: Cross-crate integration and E2E-style tests.
- `crates/main/`: Binary entrypoint for running the app.
- `config/`: Default runtime config (`default.yml`).
- `migrations/`: SQL migrations.
- `scripts/`, `perf/`: Helpers and performance scripts.

## Build, Test, and Development Commands

- Build all: `cargo build --workspace`
- Lint/format: `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`
- Unit/integration tests: `cargo test --workspace`
- Run app: `cargo run -p main`
- Infra services (optional): `docker-compose up -d` (Postgres/Redis/Kafka for infra tests)

## Coding Style & Naming Conventions

- Rust 2021 defaults; 4-space indent; keep modules small and focused.
- Names: `snake_case` (functions/files), `CamelCase` (types), `SCREAMING_SNAKE_CASE` (consts).
- Keep public APIs minimal; prefer explicit `pub use` over glob re-exports.
- Use `rustfmt` and `clippy` before committing.

## Testing Guidelines

- Framework: `cargo test` with `tokio` for async cases.
- Locations: unit tests near modules (`#[cfg(test)]`), integration in `crates/tests/` and `crates/web-api/tests/`.
- Some E2E tests require running services (Kafka/Redis/Postgres); skip or run with Docker.
- Naming: suffix `_tests.rs`; keep tests deterministic and isolated.

## Commit & Pull Request Guidelines

- Commit style: Conventional Commits (e.g., `feat: ...`, `fix: ...`, `refactor: ...`).
- PRs: include a clear description, linked issues, how-to-test steps, and any config changes.
- Add or update tests for behavior changes; run `cargo test --workspace` before requesting review.

## Security & Configuration Tips

- Local development works in-memory; infra-backed features read from `config/default.yml` and env (e.g., `ENABLE_ORGANIZATIONS`).
- Do not commit secrets; prefer env vars or a local, untracked overrides file.
- When enabling infra, ensure Docker services are healthy before running tests.

## Architecture Overview

- CQRS layering: Domain → Application (CQRS/services) → Infrastructure adapters → Web API.
- Prefer domain-driven logic in `crates/domain`; keep adapters replaceable and side-effectful code in `crates/infrastructure/`.
