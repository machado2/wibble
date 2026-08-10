import { ImageRepository } from "@/core/ImageRepository";
import { ImageService } from "./ImageService";

class ImageGenerationTask {
  private imageRepository = new ImageRepository();
  private imageService = new ImageService();

  public async run(): Promise<void> {
    const image = await this.imageRepository.getNextImageToGenerate();
    if (image) {
      const prompt = image.prompt;
      await this.imageService.generateImage(prompt, image.id);
    } else {
      // sleep for 30 seconds
      await new Promise((resolve) => setTimeout(resolve, 30000));
    }
  }
}

export const imageGenerationLoop = async () => {
  console.log("Starting image generation loop");
  for (;;) {
    try {
      const task = new ImageGenerationTask();
      await task.run();
    } catch (error: any) {
      console.log(`Image Generation failed: ${error}`);
      await new Promise((resolve) => setTimeout(resolve, 30000));
    }
  }
};
