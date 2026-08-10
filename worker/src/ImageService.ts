import { ImageRepository } from "./ImageRepository";
import { ImageGenerator } from "./ImageGenerator";
import { image_cache } from "@prisma/client";
import {
  ContentModerationError,
  PermanentImageGenerationError,
} from "./errors";

export class ImageService {
  private repository = new ImageRepository();
  private generator = new ImageGenerator();

  public async generateImage(image: image_cache): Promise<void> {
    const prompt = image.prompt;
    const id = image.id;
    if (prompt === null) {
      await this.repository.flagImage(image);
      return;
    }
    if (prompt.trim().length === 0) {
      await this.repository.flagImage(image);
      return;
    }
    await this.repository.markGenerating(id);
    try {
      const imageData = await this.generator.generateImage(image);
      await this.repository.updateEntryWithImage(id, imageData);
    } catch (error) {
      if (
        error instanceof ContentModerationError ||
        error instanceof PermanentImageGenerationError
      ) {
        await this.repository.failPermanently(id, String(error));
        return;
      }
      await this.repository.flagImage(image, String(error));
      throw error;
    }
  }
}
