import type { WriterConfig } from "../../../config-runtime";

export class WriterSelectionError extends Error {
  constructor(message: string, public readonly statusCode: number) {
    super(message);
    this.name = "WriterSelectionError";
  }
}

export const eligibleWriters = (
  writers: WriterConfig[],
  admin: boolean
): WriterConfig[] =>
  writers.filter((writer) => writer.available && (!writer.admin_only || admin));

export const selectWriter = (
  writers: WriterConfig[],
  requestedId: string | undefined,
  admin: boolean,
  random: () => number = Math.random
): WriterConfig => {
  const eligible = eligibleWriters(writers, admin);
  if (requestedId) {
    const configured = writers.find((writer) => writer.id === requestedId);
    if (!configured || !configured.available) {
      throw new WriterSelectionError("Writer is not available", 400);
    }
    if (configured.admin_only && !admin) {
      throw new WriterSelectionError("Writer is available only to admins", 403);
    }
    return configured;
  }
  if (eligible.length === 0) {
    throw new WriterSelectionError("No writers are currently available", 503);
  }
  const index = Math.min(
    eligible.length - 1,
    Math.floor(Math.max(0, random()) * eligible.length)
  );
  return eligible[index];
};

export type PublicWriter = Pick<WriterConfig, "id" | "nickname">;
