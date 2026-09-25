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

describe("ContentGenerator article translations", () => {
  const writer = {
    id: "luna",
    nickname: "Luna",
    slug: "gpt-5.6-luna",
    provider: "openai",
    available: true,
    admin_only: false,
  } as const;

  afterEach(() => jest.restoreAllMocks());

  test("translates visible article text while preserving generated image directives", async () => {
    const generator = new ContentGenerator();
    const imageDirective =
      '<GeneratedImage prompt="keep this exact prompt" alt="Original caption" />';
    const ask = jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "Título traduzido",
        description: "Descrição traduzida",
        content: "Texto traduzido\n\n{{WIBBLE_PRESERVE_0}}",
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original title",
          description: "Original description",
          content: `Original text\n\n${imageDirective}`,
        },
        "Portuguese (Brazil)"
      )
    ).resolves.toEqual({
      title: "Título traduzido",
      description: "Descrição traduzida",
      content: `Texto traduzido\n\n${imageDirective}`,
    });

    const prompt = ask.mock.calls[0][1];
    expect(prompt).toContain("Portuguese (Brazil)");
    expect(prompt).toContain("{{WIBBLE_PRESERVE_0}}");
    expect(prompt).not.toContain("keep this exact prompt");
  });

  test("preserves Markdown images without exposing their URLs to the model", async () => {
    const generator = new ContentGenerator();
    const markdownImage =
      "![Original alt](https://example.com/image_(final).png)";
    const ask = jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "Título",
        description: "Descrição",
        content: "Texto traduzido\n\n{{WIBBLE_PRESERVE_0}}",
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: `Original text\n\n${markdownImage}`,
        },
        "Portuguese (Brazil)"
      )
    ).resolves.toMatchObject({
      content: `Texto traduzido\n\n${markdownImage}`,
    });
    expect(ask.mock.calls[0][1]).not.toContain("example.com");
  });

  test("preserves shortcut Markdown image references and their definitions", async () => {
    const generator = new ContentGenerator();
    const markdownImage =
      "![Original alt]\n\n[Original alt]: https://example.com/shortcut.png";
    const ask = jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "Título",
        description: "Descrição",
        content:
          "Texto traduzido\n\n{{WIBBLE_PRESERVE_0}}\n\n{{WIBBLE_PRESERVE_1}}",
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: `Original text\n\n${markdownImage}`,
        },
        "Portuguese (Brazil)"
      )
    ).resolves.toMatchObject({
      content: `Texto traduzido\n\n${markdownImage}`,
    });
    expect(ask.mock.calls[0][1]).not.toContain("example.com");
  });

  test("rejects translated fields that exceed persistence limits", async () => {
    const generator = new ContentGenerator();
    jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "x".repeat(501),
        description: "Translated",
        content: "Translated",
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        { title: "Original", description: "Original", content: "Original" },
        "English"
      )
    ).rejects.toThrow();
  });

  test("rejects new executable MDX from a model response", async () => {
    const generator = new ContentGenerator();
    jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "Translated",
        description: "Translated",
        content: "Safe text {globalThis.alert('x')} <script>alert('x')</script>",
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: "Original body",
        },
        "English"
      )
    ).rejects.toThrow();
  });

  test("rejects dangerous link protocols from a model response", async () => {
    const generator = new ContentGenerator();
    jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "Translated",
        description: "Translated",
        content: "[Click me](javascript:alert(document.domain))",
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: "[Safe](https://example.com)",
        },
        "English"
      )
    ).rejects.toThrow();
  });

  test.each([
    "[Click](jav&#x61;script:alert(1))",
    "[Click](java&Tab;script:alert(1))",
    "[Click][unsafe]\n\n[unsafe]:\njavascript:alert(1)",
    "[Click][unsafe]\n\n[unsafe]:\r\n\tjavascript:alert(1)",
    "[Click][unsafe]\n\n[unsafe]:\n    javascript:alert(1)",
    "[Click][a\\]]\n\n[a\\]]: javascript:alert(1)",
  ])("rejects obfuscated dangerous Markdown destinations: %s", async (content) => {
    const generator = new ContentGenerator();
    jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "Translated",
        description: "Translated",
        content,
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: "[Safe](https://example.com)",
        },
        "English"
      )
    ).rejects.toThrow();
  });

  test("rejects executable spreads in a preserved GeneratedImage", async () => {
    const generator = new ContentGenerator();
    const ask = jest.spyOn(generator, "askGpt");

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: '<GeneratedImage prompt="safe" {...globalThis} />',
        },
        "English"
      )
    ).rejects.toThrow();
    expect(ask).not.toHaveBeenCalled();
  });

  test("rejects source MDX other than the supported image component", async () => {
    const generator = new ContentGenerator();
    const ask = jest.spyOn(generator, "askGpt");

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: "<DangerousComponent payload={globalThis} />",
        },
        "English"
      )
    ).rejects.toThrow();
    expect(ask).not.toHaveBeenCalled();
  });

  test("rejects a translation that drops protected article directives", async () => {
    const generator = new ContentGenerator();
    jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "Translated",
        description: "Translated",
        content: "The protected block disappeared",
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: '<GeneratedImage prompt="keep" alt="caption" />',
        },
        "English"
      )
    ).rejects.toThrow();
  });

  test("translates content containing bare --> arrows in code spans", async () => {
    const generator = new ContentGenerator();
    const ask = jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "Traduzido",
        description: "Traduzido",
        content: "Texto `torradeira --> prefeitura --> café` traduzido",
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: "Original `toaster --> cityhall --> breakfast` text",
        },
        "Portuguese (Brazil)"
      )
    ).resolves.toMatchObject({
      content: "Texto `torradeira --> prefeitura --> café` traduzido",
    });
    expect(ask).toHaveBeenCalledTimes(1);
  });

  test("still rejects HTML comments in a model response", async () => {
    const generator = new ContentGenerator();
    jest.spyOn(generator, "askGpt").mockResolvedValue(
      JSON.stringify({
        title: "Translated",
        description: "Translated",
        content: "Safe <!-- hidden --> text",
      })
    );

    await expect(
      generator.generateTranslation(
        writer,
        {
          title: "Original",
          description: "Original",
          content: "Original body",
        },
        "English"
      )
    ).rejects.toThrow();
  });
});

describe("ContentGenerator OpenRouter requests", () => {
  afterEach(() => {
    jest.restoreAllMocks();
    mockGetWibbleConfig.mockReset();
  });

  test("disables reasoning so the output budget cannot be spent thinking", async () => {
    mockGetWibbleConfig.mockReturnValue({
      app: { site_url: "https://wibble.fbmac.net" },
      secrets: { openrouter_api_key: "test-key", openai_api_key: "" },
      generation: {
        moderation_enabled: false,
        openrouter_api_url: "https://openrouter.ai/api/v1/chat/completions",
        openai_api_url: "",
        max_output_tokens: 16000,
      },
    } as ReturnType<typeof getWibbleConfig>);
    const fetchSpy = jest.spyOn(global, "fetch").mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ choices: [{ message: { content: "resposta" } }] }),
    } as Response);

    const generator = new ContentGenerator();
    await expect(
      generator.askGpt(
        {
          id: "deepseek-flash",
          nickname: "DeepSeek Flash",
          slug: "deepseek/deepseek-v4-flash-0731",
          provider: "openrouter",
          available: true,
          admin_only: false,
        },
        "prompt",
        undefined,
        true
      )
    ).resolves.toBe("resposta");

    const body = JSON.parse(
      String((fetchSpy.mock.calls[0][1] as RequestInit).body)
    );
    expect(body.reasoning).toEqual({ enabled: false });
    expect(body.model).toBe("deepseek/deepseek-v4-flash-0731");
  });
});
