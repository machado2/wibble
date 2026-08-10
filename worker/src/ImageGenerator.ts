import { GeneratedImageData } from "./ContentGenerator";
import {
  ExternalServiceError,
  PermanentImageGenerationError,
} from "./errors";
import { Config } from "./config";
import { Sleep } from "./sleep";
import { image_cache } from "@prisma/client";

let lastReplicateRequestAt = 0;

const isPermanentProviderResponse = (status: number) =>
  status >= 400 &&
  status < 500 &&
  ![408, 409, 425, 429].includes(status);

export class ImageGenerator {
  async generateImage(image: image_cache): Promise<GeneratedImageData> {
    if (Config.imageMode !== "replicate") {
      throw new ExternalServiceError(
        `Unsupported IMAGE_MODE for the old worker: ${Config.imageMode}`
      );
    }

    const prompt = image.prompt;
    const waitMs =
      Config.replicateMinRequestIntervalSeconds * 1000 -
      (Date.now() - lastReplicateRequestAt);
    if (waitMs > 0) {
      await Sleep(waitMs / 1000);
    }
    lastReplicateRequestAt = Date.now();
    const request = { input: { prompt } };
    const response = await fetch(Config.replicateApiUrl, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${Config.replicateApiToken}`,
        Accept: "application/json",
        "Content-Type": "application/json",
      },
      body: JSON.stringify(request),
    });
    const created = await response.json().catch(() => null);
    if (!response.ok) {
      const ErrorType = isPermanentProviderResponse(response.status)
        ? PermanentImageGenerationError
        : ExternalServiceError;
      throw new ErrorType(`Replicate request failed with HTTP ${response.status}`);
    }

    const predictionUrl =
      created?.urls?.get ??
      (created?.id
        ? `${new URL(Config.replicateApiUrl).origin}/v1/predictions/${
            created.id
          }`
        : undefined);
    if (!predictionUrl) {
      throw new ExternalServiceError(
        "Replicate response did not include a polling URL"
      );
    }

    for (let attempt = 0; attempt < 120; attempt++) {
      await Sleep(2);
      const pollResponse = await fetch(predictionUrl, {
        headers: {
          Authorization: `Bearer ${Config.replicateApiToken}`,
          Accept: "application/json",
        },
      });
      const prediction = await pollResponse.json().catch(() => null);
      if (!pollResponse.ok) {
        const ErrorType = isPermanentProviderResponse(pollResponse.status)
          ? PermanentImageGenerationError
          : ExternalServiceError;
        throw new ErrorType(
          `Replicate polling failed with HTTP ${pollResponse.status}`
        );
      }
      if (
        prediction?.status === "failed" ||
        prediction?.status === "canceled"
      ) {
        throw new PermanentImageGenerationError(
          `Replicate generation ${prediction.status}`
        );
      }
      if (prediction?.status !== "succeeded") {
        continue;
      }

      const urlImage = Array.isArray(prediction.output)
        ? prediction.output[0]
        : prediction.output;
      if (typeof urlImage !== "string" || urlImage.length === 0) {
        throw new PermanentImageGenerationError(
          "Replicate generation returned no image"
        );
      }
      const imageResponse = await fetch(urlImage);
      if (!imageResponse.ok) {
        throw new ExternalServiceError(
          `Failed to download generated image: HTTP ${imageResponse.status}`
        );
      }
      const imageBuffer = Buffer.from(await imageResponse.arrayBuffer());
      const contentType = imageResponse.headers.get("content-type") ?? "";
      const extension = contentType.includes("png")
        ? ".png"
        : contentType.includes("jpeg")
        ? ".jpg"
        : ".webp";
      return {
        prompt,
        model: Config.replicateApiUrl,
        image: imageBuffer,
        generator: "replicate",
        seed: "",
        parameters: JSON.stringify(request),
        extension,
      };
    }
    throw new ExternalServiceError("Replicate generation timed out");
  }
}
