import { ContentGenerator } from "./ContentGenerator";

describe("ContentGenerator moderation feature flag", () => {
  const originalFlag = process.env.OPENAI_MODERATION_ENABLED;

  afterEach(() => {
    jest.restoreAllMocks();
    if (originalFlag === undefined) {
      delete process.env.OPENAI_MODERATION_ENABLED;
    } else {
      process.env.OPENAI_MODERATION_ENABLED = originalFlag;
    }
  });

  test("does not call the moderation API when the flag is absent", async () => {
    delete process.env.OPENAI_MODERATION_ENABLED;
    const fetchSpy = jest.spyOn(global, "fetch");
    const generator = new ContentGenerator();

    await expect(generator.moderateContent("test input")).resolves.toBe(true);
    expect(fetchSpy).not.toHaveBeenCalled();
  });
});
