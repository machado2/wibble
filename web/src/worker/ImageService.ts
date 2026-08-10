import { ImageRepository } from "@/core/ImageRepository";
import { ContentGenerator } from "./ContentGenerator";

export class ImageService {
  private repository = new ImageRepository();
  private generator = new ContentGenerator();

  public async generateImage(prompt: string | null, id: string): Promise<void> {
    if (prompt === null) {
      this.repository.flagImage(id);
      return;
    }
    if (prompt.trim().length === 0) {
      this.repository.flagImage(id);
      return;
    }
    const imageData = await this.generator.generateImage(prompt);
    if (imageData === null) throw new Error("Image generation failed");
    await this.repository
      .updateEntryWithImage(id, imageData)
      .catch((error: any) => {
        console.log(
          `Image generation failed:, prompt: ${prompt}, id: ${id}, error: ${error}`
        );
        const status = parseInt(error?.response?.status) || 500;
        if (status === 429) {
          console.log("API rate limit exceeded");
          // wait 5 minutes
          setTimeout(() => this.repository.flagImage(id), 300000);
        } else {
          console.log(
            `Image generation flagged:, prompt: ${prompt}, id: ${id}, error: ${error}`
          );
          this.repository.flagImage(id);
        }
      });
  }
}
