import { getExampleArticles } from "./ExampleArticles";
import textGenerator from "./TextGenerator";
import fs from "fs";
import path from "path";

const systemMessageTemplate = fs.readFileSync(
  path.join(process.cwd(), "system_message.txt"),
  "utf8"
);

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
  parameters: string;
  extension: ".jpg" | ".png" | ".webp";
};

const TAG_START = "<<<START>>>";
const TAG_END = "<<<END>>>";

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

export class ContentGenerator {
  constructor(private model: string) {}

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

  public async generateContent(
    title: string,
    description: string,
    userInput: string
  ): Promise<string> {
    const exampleArticles = await getExampleArticles();

    let systemMessage = systemMessageTemplate;

    if (this.model !== "gpt-4") {
      systemMessage += `

    Examples:

    <<<START OF EXAMPLE 1>>>${exampleArticles[0]}<<<END OF EXAMPLE 1>>>
    <<<START OF EXAMPLE 2>>>${exampleArticles[1]}<<<END OF EXAMPLE 2>>>

    `;
    }

    let instructions: string;
    try {
      instructions = JSON.parse(userInput).suggestion;
    } catch (error: any) {
      instructions = userInput;
    }

    const prompt = `${this.wrapUserInput("title for the article", title)}
${this.wrapUserInput("description for the article", description)}
${this.wrapUserInput("instructions", instructions)}`;

    return await textGenerator.askGpt(this.model, prompt, systemMessage);
  }
}
