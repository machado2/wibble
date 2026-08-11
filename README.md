# Wibble monorepo

This repository contains the three Wibble implementations:

- `web/`: the Node.js/Next.js public application currently deployed at
  `wibble.fbmac.net`;
- `worker/`: the Node.js background article and image generator used by the
  public application;
- `rust/`: the newer Rust implementation retained as a separate application.

The Node.js applications share a single pnpm workspace and lockfile. Install
and validate them from the repository root:

```bash
pnpm install
pnpm typecheck
pnpm build
```

Text generation uses OpenRouter when `OPENROUTER_API_KEY` is set; otherwise it
falls back to `OPENAI_API_KEY`. The model is selected with `LANGUAGE_MODEL`.
The optional OpenAI moderation pre-check is disabled by default and can be
enabled explicitly with `OPENAI_MODERATION_ENABLED=true` and an
`OPENAI_API_KEY`.

Run Rust commands from `rust/`:

```bash
cd rust
cargo check
cargo test
```

Each application keeps its own README, Dockerfile, schema, and license.
