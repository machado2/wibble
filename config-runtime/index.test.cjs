const assert = require("node:assert/strict");
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
    path.join(os.tmpdir(), "wibble-config-test-"),
  );
  const temporaryConfig = path.join(temporaryDirectory, "config.ncl");
  const originalConsoleError = console.error;

  try {
    fs.copyFileSync(path.resolve(__dirname, "../config.ncl"), temporaryConfig);
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
