import fs from "fs/promises";
import prisma from "../core/PrismaWibble";
import { ImageRepository } from "../core/ImageRepository";
import { imageHandler } from "./ImageForward";

jest.mock("fs/promises", () => ({
  __esModule: true,
  default: { readFile: jest.fn() },
}));
jest.mock("../core/PrismaWibble", () => ({
  __esModule: true,
  default: {
    image_file: { findUnique: jest.fn() },
    image_cache: { update: jest.fn() },
  },
}));
jest.mock("../core/ImageRepository", () => ({
  ImageRepository: jest.fn(),
}));
jest.mock("../../../config-runtime", () => ({
  getWibbleConfig: () => ({
    app: { images_dir: "/data/images" },
    secrets: { replicate_api_token: "" },
  }),
}));

describe("imageHandler", () => {
  test("does not report the placeholder as a completed generated image", async () => {
    const image = {
      id: "generated-image-id",
      status: "completed",
      flagged: false,
      provider_job_url: null,
    };
    (ImageRepository as jest.Mock).mockImplementation(() => ({
      getImageById: jest.fn().mockResolvedValue(image),
    }));
    (prisma.image_file.findUnique as jest.Mock).mockResolvedValue({
      id: image.id,
      file_path: `${image.id}.webp`,
    });
    (fs.readFile as jest.Mock).mockRejectedValue(
      Object.assign(new Error("missing image"), { code: "ENOENT" })
    );

    const request = {
      method: "GET",
      query: { id: image.id },
    } as any;
    const response = {
      setHeader: jest.fn(),
      status: jest.fn().mockReturnThis(),
      send: jest.fn().mockReturnThis(),
    } as any;

    await imageHandler(request, response);

    expect(response.status).toHaveBeenCalledWith(404);
    expect(response.send).toHaveBeenCalledWith("Image data not found");
    expect(fs.readFile).toHaveBeenCalledTimes(1);
    expect(prisma.image_cache.update).not.toHaveBeenCalled();
  });
});
