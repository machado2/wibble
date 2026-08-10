# Repository Guidelines

## Layout

- `web/` contains the deployed Next.js application.
- `worker/` contains the deployed Node.js generation worker.
- `rust/` contains an experimental, not used, Rust implementation.
- `.github/` and the pnpm workspace configuration are repository-wide.

## Commands

Use pnpm, never npm. Run Node.js workspace commands from the repository root:

- `pnpm install`
- `pnpm typecheck`
- `pnpm build`
- `pnpm --filter wibble-web dev`
- `pnpm --filter wibble-worker start`

Run Rust commands from `rust/`:

- `cargo check`
- `cargo test`
- `cargo fmt`
- `cargo clippy -- -D warnings`

## Git

Use the `master` branch. Do not create branches unless explicitly requested.
For `github.com/machado2`, commit directly to `master`. Never commit secrets,
local databases, generated builds, or dependency directories.
