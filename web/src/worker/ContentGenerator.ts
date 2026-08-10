import {
  ContentModerationError,
  ExternalServiceError,
  InvalidGptResponseError,
  RateLimitError,
} from "@/core/errors";
import { stableHordeClient } from "../generated/stable-horde-api";
import axios from "axios";
import sharp from "sharp";
import slugify from "slugify";
import prisma from "@/core/PrismaWibble";
import { v4 as uuidv4 } from "uuid";

export type TitleDescription = {
  slug: string;
  title: string;
  description: string;
};
export type TitleList = TitleDescription[];

export type GeneratedImageData = {
  prompt: string;
  model: string;
  image: Buffer;
  generator: string;
  seed: string;
};

function escapeMarkdownString(str: string): string {
  const mdSpecialCharacters = [
    "`",
    "*",
    "_",
    "{",
    "}",
    "[",
    "]",
    "(",
    ")",
    "#",
    "+",
    "-",
    ".",
    "!",
    "|",
  ];
  let escapedString = "";

  for (let char of str) {
    if (mdSpecialCharacters.includes(char)) {
      escapedString += "\\" + char;
    } else {
      escapedString += char;
    }
  }

  return escapedString;
}

const responseText = (response: any): string | null => {
  if (typeof response?.output_text === "string" && response.output_text) {
    return response.output_text;
  }
  const text = response?.output
    ?.filter((item: any) => item?.type === "message")
    ?.flatMap((item: any) => item?.content ?? [])
    ?.filter((item: any) => item?.type === "output_text")
    ?.map((item: any) => item?.text ?? "")
    ?.join("");
  return text || null;
};

export class ContentGenerator {
  private openAiApiKey =
    process.env.OPENAI_API_KEY ?? process.env.OPENAI_KEY ?? "";
  private openAiApiUrl =
    process.env.OPENAI_API_URL ?? "https://api.openai.com/v1/responses";
  private _model = (
    process.env.LANGUAGE_MODEL ??
    process.env.OPENAI_MODEL ??
    process.env.TEXT_MODEL ??
    "gpt-3.5-turbo"
  )
    .split(",")[0]
    .trim();

  get model(): string {
    return this._model;
  }

  // check if a content exists with the slug
  async checkSlug(slug: string): Promise<boolean> {
    const response = await prisma.content.findUnique({
      where: { slug },
      select: { id: true },
    });
    return response != null;
  }

  async createSlug(title: string) {
    let slug = slugify(title, { lower: true, strict: true });
    if (slug.length < 1) {
      slug = uuidv4();
    }
    if (!(await this.checkSlug(slug))) {
      return slug;
    }
    for (let i = 0; ; i++) {
      const slug_i = slug + "-" + i;
      if (!(await this.checkSlug(slug_i))) {
        return slug_i;
      }
    }
  }

  async askGpt(model: string, prompt: string): Promise<string> {
    if (!(await this.moderateContent(prompt))) {
      throw new ContentModerationError();
    }

    const response = await fetch(this.openAiApiUrl, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${this.openAiApiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        model,
        input: [{ role: "user", content: prompt }],
        reasoning: { effort: "none" },
        max_output_tokens: 16000,
      }),
    });
    const payload = await response.json().catch(() => null);
    if (response.status === 429) {
      throw new RateLimitError();
    }
    if (!response.ok) {
      throw new ExternalServiceError(
        `OpenAI request failed with HTTP ${response.status}: ${JSON.stringify(
          payload
        )}`
      );
    }
    const content = responseText(payload);
    if (content == null) {
      throw new ExternalServiceError();
    }
    console.log(`Asked GPT: ${prompt}, got: ${content}`);
    return content;
  }

  wrapUserInput(what: string, text: string | null) {
    // Sanitize by replacing delimiters if they exist in user's input
    if (!text) {
      return "";
    }
    const sanitized = escapeMarkdownString(text);
    const delimited = "```\n" + sanitized + "```\n";
    const wrapped = `This is the ${what}:\n\n: ${delimited}\n\n`;
    return wrapped;
  }

  async generateTitleAndDescription(
    model: string,
    suggestion: string | null
  ): Promise<TitleDescription> {
    const prompt = `You are a professional content writer for "The Wibble", a satirical news website that generates
    articles.

    Your task is to create a title and description for an article
    following the instructions provided from a user.

    * The description shouldn't spoil the article, don't spoil the punchline or any other surprise in the description and title.
    * The description should be introductory, not a summary of the article. Something like the first sentence of the article.
    * If there is a twist in the article, don't even hint it at the description.
    * Show, don't tell. Don't use the words like "satire", "irony" or "humor".

    ${this.wrapUserInput("user instructions", suggestion)}

    Your response must be a JSON object, following strictly the format:

    {"title": "Title of the article", "description": "Description of the article"}

    Important that your response must be only a JSON, as it needs to be parsed by the system.
    Any other text will be considered invalid.
    `;

    const gptResponse = await this.askGpt(model, prompt);
    try {
      const parsed = JSON.parse(gptResponse);
      return {
        slug: await this.createSlug(parsed.title),
        title: parsed.title,
        description: parsed.description,
      };
    } catch (e) {
      throw new InvalidGptResponseError();
    }
  }

  public async generateContent(
    model: string,
    title: string,
    description: string,
    userInput: string
  ): Promise<string> {
    const prompt = `You are a professional content writer for "The Wibble", a satirical news website that generates
articles based on the URL slugs entered by users. Your task is to create a captivating and humorous article.

The title for the article you'll create is "${title}". And the description is:

${this.wrapUserInput("title for the article", title)}
${this.wrapUserInput("description for the article", description)}
${this.wrapUserInput("additional user instructions/keywords/prompt", userInput)}

It is important that your response must be only the article content, as it needs to be parsed by the system. Any other
text will be considered invalid.

You must follow strictly these guidelines:

* Don't include the title or description in the article, that is already provided.
* As a satirical news platform, ensure your article is engaging and amusing.
* Refrain from adding disclaimers, signatures, categories, or any elements unrelated to the main content.
* Ensure the article is between 800 and 2000 words in length.
* The content should be appropriate for a general audience. If the user input is inappropriate, you can deviate from it.
* Use markdown mixed with XML format for the article. The only XML tag allowed is the <GeneratedImage>.
* Include one or more images in the content, using the component <GeneratedImage>. Images will be rendered
on the full width of the page. Use detailed prompts for image generation and follow strictly the format
demonstrated here. These are examples, don't use them in your article:

<GeneratedImage prompt="A dachshund working on the coffee maker in the morning in the kitchen, masterpiece, best quality, high quality, extremely detailed cg unity 8k wallpaper, scenery, award winning photography, hdr, trending on artstation, trending on cgsociety, intricate, high detail, art by midjourney" alt="Dachshund making coffee" />

<GeneratedImage prompt="A digital painting of an Alien scene with a strange alien creature walking on its eight legs to the horizon, mist in the background, strange alien planet, Realistic, Well-rendered, Artistic, Imaginative, Detailed, Expressive, Dynamic, Evocative, Masterful, Stylish" alt="Alien creature on its planet" />

<GeneratedImage prompt="photo of one 29 year old woman, pale skin, homeless in new york city, upper body, dark hair, detailed skin, detailed eyes, realistic eyes, 20 megapixel, canon eos r3, detailed skin, detailed face" alt="homeless woman" />

* Don't use the markdown image syntax.
* Don't use any external links.
* The place where the <GeneratedImage> is written determines the place the image will be rendered in the article.
* Don't make any direct reference to the article being satire or funny, as it breaks the suspension of disbelief.
* Don't use the example images provided in the prompts, as they are only for demonstration purposes.
* Don't use any other XML tags other than <GeneratedImage>.
* Don't reference things from these guidelines, as they aren't known by our readers and wouldn't make sense in the article.
* Show, don't tell. Don't use the words like "satire", "irony" or "humor" in the article.
`;

    return await this.askGpt(model, prompt);
  }

  async moderateContent(content: string): Promise<boolean> {
    const moderationUrl = this.openAiApiUrl.replace(
      /\/responses?\/?$/,
      "/moderations"
    );
    const response = await fetch(moderationUrl, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${this.openAiApiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ input: content }),
    });
    const payload = await response.json().catch(() => null);
    if (response.status === 429) {
      throw new RateLimitError();
    }
    if (!response.ok) {
      throw new ExternalServiceError(
        `OpenAI moderation failed with HTTP ${
          response.status
        }: ${JSON.stringify(payload)}`
      );
    }
    const moderation_result = payload?.results?.[0]?.flagged === true;
    if (moderation_result) {
      console.log(`Content flagged: ${content}`);
    }
    return !moderation_result;
  }

  async generateImageDalle(prompt: string): Promise<Buffer | null> {
    const response = await fetch(
      process.env.OPENAI_IMAGE_API_URL ??
        "https://api.openai.com/v1/images/generations",
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${
            process.env.OPENAI_API_KEY ?? process.env.OPENAI_KEY ?? ""
          }`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ prompt, n: 1, size: "512x512" }),
      }
    );
    if (!response.ok) {
      throw new ExternalServiceError(
        `Image generation failed with HTTP ${response.status}`
      );
    }
    const responseData = await response.json();
    const imageUrl = responseData.data?.[0]?.url as string | undefined;
    if (!imageUrl) {
      throw new ExternalServiceError("Image generation returned no image URL");
    }
    const imageDataResponse = await axios.get(imageUrl, {
      responseType: "arraybuffer",
    });
    const imageBuffer = Buffer.from(imageDataResponse.data, "binary");
    const jpegImageBuffer = await sharp(imageBuffer)
      .jpeg({ quality: 85 })
      .toBuffer();
    return jpegImageBuffer;
  }

  async generateImageAuto1111(prompt: string): Promise<Buffer | null> {
    const payload = { prompt: prompt };
    const response = await axios.post(
      "http://127.0.0.1:7860/sdapi/v1/txt2img",
      payload
    );
    const imageUrl = response.data.images[0] as string;
    // Decode the base64 image data and convert it to a buffer
    const imageData = Buffer.from(imageUrl.split(",", 1)[0], "base64");
    // Convert the image to jpeg format with 85% quality
    const jpegImageBuffer = await sharp(imageData)
      .jpeg({ quality: 85 })
      .toBuffer();
    return jpegImageBuffer;
  }

  async generateImageopenapi(prompt: string): Promise<Buffer | null> {
    const stableHorde = new stableHordeClient({
      BASE: "https://stablehorde.net/api/",
    });
    const apikey = process.env.SDH_KEY;
    if (!apikey) {
      throw new Error("No API key for stable horde configured");
    }
    const generation = await stableHorde.v2.postImageAsyncGenerate(apikey, {
      prompt,
    });
    const id = generation.id as string;
    for (let attempt = 0; attempt < 100; attempt++) {
      // wait for 10 seconds
      await new Promise((resolve) => setTimeout(resolve, 10000));
      const generationResult = await stableHorde.v2.getImageAsyncStatus(id);
      if (generationResult.faulted) {
        throw new Error(
          `Generation for ${prompt} failed: ${JSON.stringify(generationResult)}`
        );
      }
      if (generationResult.done) {
        if (generationResult?.generations?.length != 1) {
          throw new Error(
            `Generation for ${prompt} failed: ${JSON.stringify(
              generationResult
            )}`
          );
        }
        const urlImage = generationResult.generations[0].img;
        if (!urlImage) {
          throw new Error(
            `Generation for ${prompt} failed: ${JSON.stringify(
              generationResult
            )}`
          );
        }
        const response = await axios.get(urlImage, {
          responseType: "arraybuffer",
        });
        const image_buffer = Buffer.from(response.data, "base64");
        const jpegImageBuffer = await sharp(image_buffer)
          .jpeg({ quality: 85 })
          .toBuffer();
        return jpegImageBuffer;
      }
    }
    console.log(`Generation for ${prompt} timed out`);
    return null;
  }

  async stableHordeGenerateAsync(prompt: string): Promise<string> {
    var myHeaders = new Headers();
    myHeaders.append("apikey", process.env.SDH_KEY as string);
    myHeaders.append("Content-Type", "application/json");

    const sdModel = process.env.SD_MODEL ?? "Deliberate";

    var raw = JSON.stringify({
      prompt: `${prompt} ### nsfw`,
      models: [sdModel],
      nsfw: false,
      censor_nsfw: true,
    });

    var requestOptions = {
      method: "POST",
      headers: myHeaders,
      body: raw,
    };

    const response = await fetch(
      "https://stablehorde.net/api/v2/generate/async",
      requestOptions
    );
    return (await response.json()).id;
  }

  async stableHordeStatus(id: string): Promise<any> {
    var myHeaders = new Headers();
    myHeaders.append("Accept", "application/json");

    var requestOptions = {
      method: "GET",
      headers: myHeaders,
    };

    const response = await fetch(
      `https://stablehorde.net/api/v2/generate/status/${id}`,
      requestOptions
    );
    return await response.json();
  }

  async generateImage(prompt: string): Promise<GeneratedImageData> {
    const apikey = process.env.SDH_KEY;
    if (!apikey) {
      throw new Error("No API key for stable horde configured");
    }
    const id = await this.stableHordeGenerateAsync(prompt);
    for (let attempt = 0; attempt < 100; attempt++) {
      // wait for 10 seconds
      await new Promise((resolve) => setTimeout(resolve, 10000));

      const generationResult = await this.stableHordeStatus(id);

      if (generationResult.faulted) {
        throw new ExternalServiceError(
          `Generation for ${prompt} failed: ${JSON.stringify(generationResult)}`
        );
      }
      if (generationResult.done) {
        if (generationResult?.generations?.length != 1) {
          throw new ExternalServiceError(
            `Generation for ${prompt} failed: ${JSON.stringify(
              generationResult
            )}`
          );
        }
        const censored = generationResult.generations[0].censored;
        if (censored) {
          throw new ExternalServiceError(
            `Generation for ${prompt} was censored: ${JSON.stringify(
              generationResult
            )}`
          );
        }
        const urlImage = generationResult.generations[0].img;
        if (!urlImage) {
          throw new ExternalServiceError(
            `Generation for ${prompt} failed: ${JSON.stringify(
              generationResult
            )}`
          );
        }
        const model = generationResult.generations[0].model ?? "";
        const seed = generationResult.generations[0].seed ?? 0;
        const imgResponse = await axios.get(urlImage, {
          responseType: "arraybuffer",
        });
        const image_buffer = Buffer.from(imgResponse.data, "base64");
        const jpegImageBuffer = await sharp(image_buffer)
          .jpeg({ quality: 85 })
          .toBuffer();
        return {
          prompt,
          model,
          image: jpegImageBuffer,
          generator: "stable horde",
          seed,
        };
      }
    }
    throw new ExternalServiceError("Generation timed out");
  }
}
