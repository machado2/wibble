export class ContentModerationError extends Error {
  constructor() {
    super("Content moderation error");
    this.name = "ContentModerationError";
  }
}

export class RateLimitError extends Error {
  constructor() {
    super("Rate limit error");
    this.name = "RateLimitError";
  }
}

export class ExternalServiceError extends Error {
  constructor(message: string = "External service error") {
    super(message);
    this.name = "ExternalServiceError";
  }
}

export class PermanentImageGenerationError extends Error {
  constructor(message: string = "Image generation failed permanently") {
    super(message);
    this.name = "PermanentImageGenerationError";
  }
}

export class InvalidGptResponseError extends ExternalServiceError {
  constructor(message: string = "Invalid GPT response") {
    super(message);
    this.name = "InvalidGptResponseError";
  }
}
