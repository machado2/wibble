import { ImageRepository } from "./ImageRepository";
import { ImageService } from "./ImageService";
import { ContentModerationError } from "./errors";
import logger from "./logger";
import { SleepOnError, SleepOnIdle } from "./sleep";

class ImageGenerationTask {
  private imageRepository = new ImageRepository();
  private imageService = new ImageService();

  public async run(): Promise<void> {
    const image = await this.imageRepository.getNextImageToGenerate();
    if (image) {
      try {
        await this.imageService.generateImage(image);
      } catch (error: any) {
        if (error?.name === "ContentModerationError") {
          await this.imageRepository.flagImage(image);
        } else {
          throw error;
        }
      }
    } else {
      await SleepOnIdle();
    }
  }
}

export const imageGenerationLoop = async () => {
  logger.info("Starting image generation loop");
  for (;;) {
    try {
      const task = new ImageGenerationTask();
      await task.run();
    } catch (error: any) {
      logger.info(`Image Generation failed: ${error}`);
      await SleepOnError();
    }
  }
};
