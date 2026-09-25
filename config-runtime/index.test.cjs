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
    fs.writeFileSync(
      temporaryConfig,
      fs.readFileSync(path.resolve(__dirname, "../config.ncl.example"), "utf8")
        .replace('translation_worker_secret = ""', 'translation_worker_secret = "test-worker-secret"')
    );
    process.chdir(temporaryDirectory);
    runtime.resetConfigCacheForTests();

    const first = runtime.getWibbleConfig();
    assert.equal(first.generation.writers[0].nickname, "DeepSeek Flash");

    const edited = fs
      .readFileSync(temporaryConfig, "utf8")
      .replace('nickname = "DeepSeek Flash"', 'nickname = "Reloaded DeepSeek"');
    fs.writeFileSync(temporaryConfig, edited);
    forceNewMtime(temporaryConfig);

    const reloaded = runtime.getWibbleConfig();
    assert.equal(reloaded.generation.writers[0].nickname, "Reloaded DeepSeek");

    fs.writeFileSync(temporaryConfig, "{ generation = { writers = [] } }");
    forceNewMtime(temporaryConfig);

    console.error = () => {};
    const retained = runtime.getWibbleConfig();
    assert.equal(retained.generation.writers[0].nickname, "Reloaded DeepSeek");
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
    const preparedNickel = path.join(temporaryDirectory, "config.ncl");
    fs.writeFileSync(
      preparedNickel,
      fs.readFileSync(path.resolve(__dirname, "../config.ncl.example"), "utf8")
        .replace('translation_worker_secret = ""', 'translation_worker_secret = "test-worker-secret"')
    );
    const exported = execFileSync(
      nickel,
      ["export", preparedNickel],
      { encoding: "utf8" }
    );
    fs.writeFileSync(temporaryConfig, exported);
    process.env.WIBBLE_CONFIG_PATH = temporaryConfig;
    runtime.resetConfigCacheForTests();

    const config = runtime.getWibbleConfig();
    assert.equal(config.app.site_url, "https://wibble.fbmac.net");
    assert.equal(config.generation.writers[0].nickname, "DeepSeek Flash");
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

test("rejects an empty translation worker secret", () => {
  const temporaryDirectory = fs.mkdtempSync(
    path.join(os.tmpdir(), "wibble-empty-worker-secret-")
  );
  const temporaryConfig = path.join(temporaryDirectory, "config.ncl");
  const originalCwd = process.cwd();

  try {
    fs.copyFileSync(
      path.resolve(__dirname, "../config.ncl.example"),
      temporaryConfig
    );
    process.chdir(temporaryDirectory);
    runtime.resetConfigCacheForTests();
    assert.throws(
      () => runtime.getWibbleConfig(),
      /translation_worker_secret must be a non-empty string/
    );
  } finally {
    process.chdir(originalCwd);
    runtime.resetConfigCacheForTests();
    fs.rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});

const loadExampleConfig = (temporaryDirectory, transform) => {
  const temporaryConfig = path.join(temporaryDirectory, "config.ncl");
  const source = fs
    .readFileSync(path.resolve(__dirname, "../config.ncl.example"), "utf8")
    .replace(
      'translation_worker_secret = ""',
      'translation_worker_secret = "test-worker-secret"'
    );
  fs.writeFileSync(temporaryConfig, transform ? transform(source) : source);
  return temporaryConfig;
};

test("loads a config without OpenAI settings", () => {
  const temporaryDirectory = fs.mkdtempSync(
    path.join(os.tmpdir(), "wibble-no-openai-")
  );
  const originalCwd = process.cwd();

  try {
    loadExampleConfig(temporaryDirectory);
    process.chdir(temporaryDirectory);
    runtime.resetConfigCacheForTests();

    const config = runtime.getWibbleConfig();
    assert.equal(config.secrets.openai_api_key, "");
    assert.equal(config.generation.openai_api_url, "");
    assert.equal(config.generation.moderation_api_url, "");
    assert.equal(config.generation.writers.length, 1);
    assert.equal(config.generation.writers[0].provider, "openrouter");
  } finally {
    process.chdir(originalCwd);
    runtime.resetConfigCacheForTests();
    fs.rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});

test("rejects an openai writer without OpenAI credentials", () => {
  const temporaryDirectory = fs.mkdtempSync(
    path.join(os.tmpdir(), "wibble-openai-writer-")
  );
  const originalCwd = process.cwd();

  try {
    loadExampleConfig(temporaryDirectory, (source) =>
      source.replace('provider = "openrouter"', 'provider = "openai"')
    );
    process.chdir(temporaryDirectory);
    runtime.resetConfigCacheForTests();
    assert.throws(
      () => runtime.getWibbleConfig(),
      /openai writers require secrets\.openai_api_key and generation\.openai_api_url/
    );
  } finally {
    process.chdir(originalCwd);
    runtime.resetConfigCacheForTests();
    fs.rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});

test("rejects moderation without OpenAI credentials", () => {
  const temporaryDirectory = fs.mkdtempSync(
    path.join(os.tmpdir(), "wibble-moderation-")
  );
  const originalCwd = process.cwd();

  try {
    loadExampleConfig(temporaryDirectory, (source) =>
      source.replace("moderation_enabled = false", "moderation_enabled = true")
    );
    process.chdir(temporaryDirectory);
    runtime.resetConfigCacheForTests();
    assert.throws(
      () => runtime.getWibbleConfig(),
      /moderation_enabled requires secrets\.openai_api_key and generation\.moderation_api_url/
    );
  } finally {
    process.chdir(originalCwd);
    runtime.resetConfigCacheForTests();
    fs.rmSync(temporaryDirectory, { recursive: true, force: true });
  }
});
