#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const [, , templateArgument, destinationArgument, ...envArguments] =
  process.argv;
if (!templateArgument || !destinationArgument || envArguments.length === 0) {
  console.error(
    "Usage: migrate-env-to-config.cjs <template> <destination> <env> [env ...]"
  );
  process.exit(1);
}

const parseEnv = (source) => {
  const parsed = {};
  for (const sourceLine of source.replace(/^\uFEFF/, "").split(/\r?\n/)) {
    const line = sourceLine.trim();
    if (!line || line.startsWith("#")) continue;
    const normalized = line.startsWith("export ") ? line.slice(7) : line;
    const separator = normalized.indexOf("=");
    if (separator < 1) continue;
    const key = normalized.slice(0, separator).trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) continue;
    let value = normalized.slice(separator + 1).trim();
    if (value.startsWith('"') && value.endsWith('"')) {
      try {
        value = JSON.parse(value);
      } catch {
        throw new Error(`Invalid double-quoted value for ${key}`);
      }
    } else if (value.startsWith("'") && value.endsWith("'")) {
      value = value.slice(1, -1);
    } else {
      value = value.replace(/\s+#.*$/, "");
    }
    parsed[key] = value;
  }
  return parsed;
};

const environment = {};
for (const envArgument of envArguments) {
  const envPath = path.resolve(envArgument);
  if (!fs.existsSync(envPath)) continue;
  Object.assign(environment, parseEnv(fs.readFileSync(envPath, "utf8")));
}

const first = (...keys) => {
  for (const key of keys) {
    if (typeof environment[key] === "string") return environment[key];
  }
  return "";
};

const values = {
  database_url: first("DATABASE_URL"),
  nextauth_secret: first("NEXTAUTH_SECRET", "SSO_CLIENT_SECRET"),
  sso_client_secret: first("SSO_CLIENT_SECRET"),
  admin_email: first("ADMIN_EMAIL", "REACT_ADMIN_EMAIL"),
  safety_identifier_secret: first(
    "SAFETY_IDENTIFIER_SECRET",
    "NEXTAUTH_SECRET"
  ),
  openai_api_key: first("OPENAI_API_KEY", "OPENAI_KEY"),
  openrouter_api_key: first("OPENROUTER_API_KEY"),
  replicate_api_token: first("REPLICATE_API_TOKEN"),
  loggly_token: first("LOGGLY_TOKEN"),
  site_url: first("SITE_URL", "NEXTAUTH_URL"),
  sso_issuer_url: first("SSO_ISSUER_URL"),
  sso_client_id: first("SSO_CLIENT_ID"),
  images_dir: first("IMAGES_DIR"),
  mode: first("IMAGE_MODE"),
};

const replaceField = (source, field, value) => {
  const expression = new RegExp(
    `(^[ \\t]*${field}[ \\t]*=[ \\t]*)"(?:\\\\.|[^"\\\\])*"([ \\t]*,)`,
    "m"
  );
  if (!expression.test(source)) {
    throw new Error(`Could not find ${field} in the Nickel template`);
  }
  return source.replace(expression, `$1${JSON.stringify(value)}$2`);
};

let output = fs.readFileSync(path.resolve(templateArgument), "utf8");
for (const [field, value] of Object.entries(values)) {
  if (value) {
    output = replaceField(output, field, value);
  }
}

const destination = path.resolve(destinationArgument);
fs.writeFileSync(destination, output, { encoding: "utf8", mode: 0o600 });
fs.chmodSync(destination, 0o600);
console.log(
  `Wrote ${destination}: ${Object.entries(values)
    .map(([field, value]) => `${field}=${value ? "set" : "empty"}`)
    .join(", ")}`
);
