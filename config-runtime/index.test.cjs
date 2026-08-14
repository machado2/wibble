const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const runtime = require("./index.cjs");

const forceNewMtime = (filePath) => {
  const next = new Date(Date.now() + 2000);
  fs.utimesSync(filePath, next, next);
};

test("reloads valid Nickel edits and retains the last valid config", () => {
  const originalCwd = process.cwd();
  const temporaryDirectory = fs.mkdtempSync(
    path.join(os.tmpdir(), "wibble-config-test-")
  );
  const temporaryConfig = path.join(temporaryDirectory, "config.ncl");
  const originalConsoleError = console.error;

  try {
    fs.copyFileSync(
      path.resolve(__dirname, "../config.ncl.example"),
      temporaryConfig
    );
    process.chdir(temporaryDirectory);
    runtime.resetConfigCacheForTests();

    const first = runtime.getWibbleConfig();
    assert.equal(first.generation.writers[0].nickname, "Luna");

    const edited = fs
      .readFileSync(temporaryConfig, "utf8")
      .replace('nickname = "Luna"', 'nickname = "Reloaded Luna"');
    fs.writeFileSync(temporaryConfig, edited);
    forceNewMtime(temporaryConfig);

    const reloaded = runtime.getWibbleConfig();
    assert.equal(reloaded.generation.writers[0].nickname, "Reloaded Luna");

    fs.writeFileSync(temporaryConfig, "{ generation = { writers = [] } }");
    forceNewMtime(temporaryConfig);

    console.error = () => {};
    const retained = runtime.getWibbleConfig();
    assert.equal(retained.generation.writers[0].nickname, "Reloaded Luna");
  } finally {
    console.error = originalConsoleError;
    process.chdir(originalCwd);
    runtime.resetConfigCacheForTests();
    fs.rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});

test("loads an exported JSON config from WIBBLE_CONFIG_PATH", () => {
  const temporaryDirectory = fs.mkdtempSync(
    path.join(os.tmpdir(), "wibble-json-config-test-")
  );
  const temporaryConfig = path.join(temporaryDirectory, "config.json");
  const originalConfiguredPath = process.env.WIBBLE_CONFIG_PATH;
  const serverNickel = "/home/fabio/.local/bin/nickel";
  const nickel = fs.existsSync(serverNickel) ? serverNickel : "nickel";

  try {
    const exported = execFileSync(
      nickel,
      ["export", path.resolve(__dirname, "../config.ncl.example")],
      { encoding: "utf8" }
    );
    fs.writeFileSync(temporaryConfig, exported);
    process.env.WIBBLE_CONFIG_PATH = temporaryConfig;
    runtime.resetConfigCacheForTests();

    const config = runtime.getWibbleConfig();
    assert.equal(config.app.site_url, "https://wibble.fbmac.net");
    assert.equal(config.generation.writers[0].nickname, "Luna");
  } finally {
    if (originalConfiguredPath === undefined) {
      delete process.env.WIBBLE_CONFIG_PATH;
    } else {
      process.env.WIBBLE_CONFIG_PATH = originalConfiguredPath;
    }
    runtime.resetConfigCacheForTests();
    fs.rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});
