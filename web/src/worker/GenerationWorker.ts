import { ContentGenerator } from "./ContentGenerator";
import { ContentRepository } from "@/core/ContentRepository";
import { serialize } from "next-mdx-remote/serialize";
import testForTitle from "@/core/testForTitle";
import {
  GeneratedImageAttributes,
  extractGeneratedImageTags,
  extractMarkdownImages,
} from "@/core/extractGeneratedImageTags";
import { ImageRepository } from "@/core/ImageRepository";
import { image_cache, content } from "@prisma/client";

class GenerationTask {
  private repository = new ContentRepository();
  private generator = new ContentGenerator();
  private containsDuplicatedTitle: boolean = false;
  private images: GeneratedImageAttributes[] = [];
  private imageRepository = new ImageRepository();

  async createContent(content: content) {
    await this.repository.startTextGeneration(content);
    return await this.generator.generateContent(
      content.model,
      content.title,
      content.description,
      content.user_input
    );
  }

  async validateGeneratedContent(
    slug: string,
    content: string
  ): Promise<boolean> {
    try {
      if (content.length < 800) {
        // too short
        await this.repository.registerContentGenerationFailure(
          slug,
          "Content too short"
        );
        return false;
      }

      if (content.startsWith("---")) {
        await this.repository.registerContentGenerationFailure(
          slug,
          "Starts with frontmatter"
        );
        return false;
      }

      // Check if serialize works
      await serialize(content);

      const markdownImages = extractMarkdownImages(content);
      if (markdownImages.length > 0) {
        await this.repository.registerContentGenerationFailure(
          slug,
          "Markdown image found"
        );
        return false;
      }

      this.containsDuplicatedTitle = testForTitle(content);

      // Check if the content contains at least one GeneratedImage with the specified format
      this.images = extractGeneratedImageTags(content);
      if (this.images.length === 0) {
        await this.repository.registerContentGenerationFailure(
          slug,
          "No image"
        );
        return false;
      }

      return true;
    } catch (error) {
      await this.repository.registerContentGenerationFailure(
        slug,
        "Error validating content"
      );
      return false;
    }
  }

  public async run(): Promise<void> {
    const content = await this.repository.getNextContentToGenerate();
    if (content) {
      const generatedContent = await this.createContent(content);
      if (!generatedContent) {
        await this.repository.finishFlagged(content.slug);
        return;
      }
      const isValid = await this.validateGeneratedContent(
        content.slug,
        generatedContent
      );
      if (!isValid) {
        await this.repository.failGeneration(content.slug);
        return;
      }
      const imageIds: image_cache[] = await Promise.all(
        this.images.map(
          async (image) =>
            await this.imageRepository.getImageByPrompt(image.prompt)
        )
      );
      const thumb = imageIds[0].id;
      await this.repository.finishTextGeneration(
        content.slug,
        generatedContent,
        this.generator.model,
        1,
        this.containsDuplicatedTitle,
        thumb,
        this.images[0].prompt
      );
    } else {
      // sleep for 30 seconds
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }
}

export const contentGenerationLoop = async () => {
  console.log("Starting content generation loop");
  for (;;) {
    try {
      const task = new GenerationTask();
      await task.run();
    } catch (error: any) {
      console.log(`Generation failed: ${error}`);
      await new Promise((resolve) => setTimeout(resolve, 30000));
    }
  }
};
