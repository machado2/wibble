#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const { getWibbleConfig } = require("../config-runtime/index.cjs");

const [, , command, ...args] = process.argv;
if (!command) {
  console.error("Usage: run-with-config.cjs <command> [args ...]");
  process.exit(1);
}

const databaseUrl = getWibbleConfig().secrets.database_url.trim();
if (!databaseUrl) {
  console.error("Missing Nickel configuration secrets.database_url");
  process.exit(1);
}

const result = spawnSync(command, args, {
  stdio: "inherit",
  env: { ...process.env, DATABASE_URL: databaseUrl },
  shell: false,
});
if (result.error) throw result.error;
process.exit(result.status ?? 1);
