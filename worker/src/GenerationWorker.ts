import { ContentGenerator } from "./ContentGenerator";
import { ContentRepository } from "./ContentRepository";
import testForTitle from "./testForTitle";
import {
  GeneratedImageAttributes,
  extractGeneratedImageTags,
  extractMarkdownImages,
} from "./extractGeneratedImageTags";
import { ImageRepository } from "./ImageRepository";
import { image_cache, content } from "@prisma/client";
import logger from "./logger";
import { SleepCoolDown, SleepOnError } from "./sleep";
import { PermanentExternalServiceError } from "./errors";

class GenerationTask {
  private generator: ContentGenerator;
  private containsDuplicatedTitle: boolean = false;
  private images: GeneratedImageAttributes[] = [];
  private imageRepository = new ImageRepository();

  constructor(private model: string, private repository: ContentRepository) {
    this.generator = new ContentGenerator(this.model);
  }

  async createContent(content: content) {
    return await this.generator.generateContent(
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
          "Content too short",
          undefined,
          content
        );
        return false;
      }

      if (content.startsWith("---")) {
        await this.repository.registerContentGenerationFailure(
          slug,
          "Starts with frontmatter",
          undefined,
          content
        );
        return false;
      }

      const markdownImages = extractMarkdownImages(content);
      if (markdownImages.length > 0) {
        await this.repository.registerContentGenerationFailure(
          slug,
          "Markdown image found",
          undefined,
          content
        );
        return false;
      }

      this.containsDuplicatedTitle = testForTitle(content);

      // Check if the content contains at least one GeneratedImage with the specified format
      this.images = extractGeneratedImageTags(content);
      if (this.images.length === 0) {
        await this.repository.registerContentGenerationFailure(
          slug,
          "No image",
          undefined,
          content
        );
        return false;
      }

      return true;
    } catch (error) {
      await this.repository.registerContentGenerationFailure(
        slug,
        "Error validating content",
        error?.toString(),
        content
      );
      return false;
    }
  }

  public async run(content: content): Promise<void> {
    const startTime = Date.now();
    const generatedContent = await this.createContent(content);
    if (!generatedContent) {
      await this.repository.finishFlagged(content.slug);
      await this.repository.registerContentGenerationFailure(
        content.slug,
        "Empty answer from createContent"
      );
      return;
    }
    const isValid = await this.validateGeneratedContent(
      content.slug,
      generatedContent
    );
    if (!isValid) {
      await this.repository.failGeneration(
        content.slug,
        "Generated content failed validation"
      );
      return;
    }
    const imageIds: image_cache[] = await Promise.all(
      this.images.map(
        async (image) =>
          await this.imageRepository.getImageByPrompt(
            image.prompt,
            content.id,
            image.alt
          )
      )
    );
    const thumb = imageIds[0].id;
    const ellapsedTimeMs = Date.now() - startTime;
    await this.repository.finishTextGeneration(
      content.slug,
      generatedContent,
      this.model,
      1,
      this.containsDuplicatedTitle,
      thumb,
      this.images[0].prompt,
      ellapsedTimeMs
    );
  }
}

export const contentGenerationLoop = async () => {
  logger.info("Starting content generation loop");
  for (;;) {
    const repo = new ContentRepository();
    let queuedContent: content | null = null;
    try {
      queuedContent = await repo.getNextContentToGenerate();
      if (queuedContent) {
        const task = new GenerationTask(queuedContent.model, repo);
        await task.run(queuedContent);
      }
    } catch (error: any) {
      logger.error(
        `Generation failed for ${queuedContent?.slug ?? "unknown job"}: ${error}`
      );
      if (queuedContent) {
        try {
          await repo.failGeneration(
            queuedContent.slug,
            error,
            error instanceof PermanentExternalServiceError
          );
        } catch (persistenceError) {
          logger.error(
            `Failed to persist generation failure for ${queuedContent.slug}: ${persistenceError}`
          );
          await SleepOnError();
        }
      } else {
        await SleepOnError();
      }
    }
    await SleepCoolDown();
  }
};
