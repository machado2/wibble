const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

test("migrates environment values into a valid private Nickel file", () => {
  const temporaryDirectory = fs.mkdtempSync(
    path.join(os.tmpdir(), "wibble-env-migration-test-")
  );
  const envPath = path.join(temporaryDirectory, ".env");
  const configPath = path.join(temporaryDirectory, "config.ncl");

  try {
    fs.writeFileSync(
      envPath,
      [
        "DATABASE_URL='postgresql://user:password@localhost:5432/wibble'",
        'NEXTAUTH_SECRET="nextauth-secret"',
        "SSO_CLIENT_SECRET=sso-secret",
        "ADMIN_EMAIL=admin@example.com",
        "OPENAI_API_KEY=openai-test-key",
        "OPENROUTER_API_KEY=openrouter-test-key",
        "REPLICATE_API_TOKEN=replicate-test-token",
        "LOGGLY_TOKEN=loggly-test-token",
        "SITE_URL=https://example.test",
        "SSO_ISSUER_URL=https://sso.example.test/api/auth",
        "SSO_CLIENT_ID=test-client-id",
        "IMAGES_DIR=/data/images",
        "IMAGE_MODE=replicate",
      ].join("\n")
    );

    execFileSync(
      process.execPath,
      [
        path.resolve(__dirname, "migrate-env-to-config.cjs"),
        path.resolve(__dirname, "../config.ncl.example"),
        configPath,
        envPath,
      ],
      { stdio: "pipe" }
    );
    const exported = JSON.parse(
      execFileSync("nickel", ["export", configPath], { encoding: "utf8" })
    );

    assert.equal(
      exported.secrets.database_url,
      "postgresql://user:password@localhost:5432/wibble"
    );
    assert.equal(exported.secrets.nextauth_secret, "nextauth-secret");
    assert.equal(exported.secrets.safety_identifier_secret, "nextauth-secret");
    assert.equal(exported.secrets.openai_api_key, "openai-test-key");
    assert.equal(exported.secrets.openrouter_api_key, "openrouter-test-key");
    assert.equal(exported.app.site_url, "https://example.test");
    assert.equal(exported.app.images_dir, "/data/images");
    assert.equal(
      exported.auth.sso_issuer_url,
      "https://sso.example.test/api/auth"
    );
    assert.equal(exported.auth.sso_client_id, "test-client-id");
    assert.equal(exported.image.mode, "replicate");
  } finally {
    fs.rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});
