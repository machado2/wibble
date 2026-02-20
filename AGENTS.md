# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the Rust application code. `src/main.rs` wires routes and middleware, while modules like `create.rs`, `content.rs`, `newslist.rs`, `image_generator/`, and `llm/` implement core features.  
`tests/` holds integration tests (for example `rate_limit_integration.rs`).  
`templates/` and `static/` provide server-rendered HTML and public assets.  
`prompts/` stores LLM prompt templates.  
`database/prisma/` contains schema and SQL migrations.

## Build, Test, and Development Commands
- `cargo run` runs the app locally (default binary: `wibble`).
- `cargo check` validates compilation quickly without producing a release binary.
- `cargo test` runs unit and integration tests.
- `cargo test --test rate_limit_integration` runs one integration suite.
- `cargo fmt` formats code using Rustfmt.
- `cargo clippy -- -D warnings` runs lints and treats warnings as errors.
- `just build-release` builds optimized binaries.
- `just dist` creates `dist/` with binary + `static/` + `templates/`.
- `cargo run --bin upload_images` migrates local images to configured S3 storage.

## Coding Style & Naming Conventions
Use standard Rust style: 4-space indentation, snake_case for functions/files/modules, PascalCase for types, and SCREAMING_SNAKE_CASE for constants.  
Prefer small modules with focused responsibilities; keep route handlers thin and move business logic into module functions.  
Run `cargo fmt` before committing.

## Testing Guidelines
Write tests close to behavior boundaries: module tests in `src/*` and integration flows in `tests/`.  
Name tests descriptively (e.g., `test_daily_burst_allows_default_and_blocks_next`).  
Cover error paths and rate-limiting/concurrency behavior for request handlers and generation flows.

## Commit & Pull Request Guidelines
Current history uses short subjects (`fix`, `limits`, `logs`). Keep subjects imperative and concise, but include scope when possible (example: `content: avoid duplicate dead-link recovery`).  
PRs should include:
- What changed and why
- Risk/rollback notes for DB or concurrency changes
- Test evidence (`cargo test`, targeted command output)
- Screenshots for template/UI changes

## Security & Configuration Tips
Never commit secrets; use `.env` for local config.  
Key env vars include `DATABASE_URL`, `IMAGES_DIR`, `STORAGE_TYPE`, and S3 credentials when using object storage.

## Runtime Behavior (Single Instance)
This service is currently deployed as a single instance. Active article generation status is authoritative in process memory, not just `content.generating` in the database.  
When touching dead-link recovery or wait-page logic, always gate "still generating" behavior on in-memory task state to avoid stale DB rows causing infinite "Generating..." pages after restarts.
