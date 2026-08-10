import { ImageRepository } from "./ImageRepository";
import { ImageGenerator } from "./ImageGenerator";
import { image_cache } from "@prisma/client";

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
    const imageData = await this.generator.generateImage(image);
    if (imageData === null) throw new Error("Image generation failed");
    await this.repository.updateEntryWithImage(id, imageData);
  }
}
