export const configuredTextModel = (): string => {
  const model = process.env.LANGUAGE_MODEL?.trim();
  if (!model) {
    throw new Error("Missing required environment variable LANGUAGE_MODEL");
  }
  return model;
};
