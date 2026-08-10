import { ImageRepository } from "./ImageRepository";
import { ImageService } from "./ImageService";
import logger from "./logger";
import { SleepOnError, SleepOnIdle } from "./sleep";

class ImageGenerationTask {
  private imageRepository = new ImageRepository();
  private imageService = new ImageService();

  public async run(): Promise<void> {
    const image = await this.imageRepository.getNextImageToGenerate();
    if (image) {
      await this.imageService.generateImage(image);
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
