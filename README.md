# Wibble monorepo

This repository contains the Wibble applications:

- `web/`: the Node.js/Next.js public application currently deployed at
  `wibble.fbmac.net`;
- `worker/`: the Node.js background article and image generator used by the
  public application;
- `rust/`: an experimental implementation that is not deployed.

The Node.js applications share a single pnpm workspace and lockfile. Install
and validate them from the repository root:

```bash
pnpm install
pnpm typecheck
pnpm build
```

Copy [`config.ncl.example`](./config.ncl.example) to the Git-ignored
`config.ncl` and fill in every setting, including credentials and provider
keys. Nickel validates the private file and exports it as JSON at runtime. Both
Node.js services reload it after the file modification time changes, without a
service restart; an invalid edit leaves the last valid configuration active.

`generation.writers` configures the available text models. Every writer has a
stable `id`, a display `nickname`, the provider model `slug`, an `available`
flag, and an `admin_only` flag. The creation API randomly chooses among writers
eligible for the current user when no writer is selected.

No application setting is read from `.env`. To migrate an existing environment
file without printing its values, run:

```bash
node deploy/migrate-env-to-config.cjs config.ncl.example config.ncl .env
```

The generated `config.ncl` is created with mode `600`. Moderation is controlled
by `generation.moderation_enabled` and defaults to disabled.

Nickel CLI 1.17.0 is required. The native server updater installs the pinned
binary with checksum verification and validates `config.ncl` before building.

Each application keeps its own README, Dockerfile, schema, and license.
