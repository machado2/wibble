import { contentGenerationLoop } from "./GenerationWorker";
import { imageGenerationLoop } from "./ImageGenerationWorker";
import express from "express";
import logger from "./logger";
import { updateScoresLoop } from "./updateHotScores";
import { translationGenerationLoop } from "./TranslationWorker";

logger.info("Starting wibble-worker...");

const app = express();
const port = 18002;

// Health check endpoint
app.get("/health", (req, res) => {
  res.status(200).send("Healthy");
});

// Start listening on the configured private worker port.
const server = app.listen(port, () => {
  logger.info(`Server is running on port ${port}`);
});

// Gracefully handle program termination
process.on("SIGTERM", () => {
  logger.info("Stopping server...");
  server.close(() => {
    logger.info("Server stopped.");
    process.exit(0);
  });
});

const promises = [
  contentGenerationLoop(),
  imageGenerationLoop(),
  translationGenerationLoop(),
  updateScoresLoop(),
];

Promise.all(promises).catch((error) => {
  logger.info(`Something failed: ${error}`);
});
