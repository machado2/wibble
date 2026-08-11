import { configuredTextModel } from "./configuredTextModel";

describe("configuredTextModel", () => {
  const originalLanguageModel = process.env.LANGUAGE_MODEL;

  afterEach(() => {
    if (originalLanguageModel === undefined) {
      delete process.env.LANGUAGE_MODEL;
    } else {
      process.env.LANGUAGE_MODEL = originalLanguageModel;
    }
  });

  test("uses the model configured by LANGUAGE_MODEL", () => {
    process.env.LANGUAGE_MODEL = "  gpt-5.6-luna  ";
    expect(configuredTextModel()).toBe("gpt-5.6-luna");
  });

  test("requires an explicit model configuration", () => {
    delete process.env.LANGUAGE_MODEL;
    expect(() => configuredTextModel()).toThrow(
      "Missing required environment variable LANGUAGE_MODEL",
    );
  });
});
