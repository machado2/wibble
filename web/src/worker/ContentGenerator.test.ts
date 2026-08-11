jest.mock("../../../config-runtime", () => ({
  getWibbleConfig: jest.fn(),
}));

import { getWibbleConfig } from "../../../config-runtime";
import { ContentGenerator } from "./ContentGenerator";

const mockGetWibbleConfig = jest.mocked(getWibbleConfig);

describe("ContentGenerator moderation feature flag", () => {
  afterEach(() => {
    jest.restoreAllMocks();
    mockGetWibbleConfig.mockReset();
  });

  test("does not call the moderation API when disabled in Nickel", async () => {
    mockGetWibbleConfig.mockReturnValue({
      generation: { moderation_enabled: false },
    } as ReturnType<typeof getWibbleConfig>);
    const fetchSpy = jest.spyOn(global, "fetch");
    const generator = new ContentGenerator();

    await expect(generator.moderateContent("test input")).resolves.toBe(true);
    expect(fetchSpy).not.toHaveBeenCalled();
  });
});
