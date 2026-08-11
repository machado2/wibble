import { createHash, createHmac } from "node:crypto";
import type { NextApiRequest } from "next";
import { getWibbleConfig } from "../../../config-runtime";

const firstHeaderValue = (value: string | string[] | undefined) => {
  const header = Array.isArray(value) ? value[0] : value;
  return header?.split(",", 1)[0]?.trim() || null;
};

export const requestIp = (req: NextApiRequest): string | null =>
  firstHeaderValue(req.headers["cf-connecting-ip"]) ??
  firstHeaderValue(req.headers["x-forwarded-for"]) ??
  req.socket.remoteAddress?.trim() ??
  null;

export const generationSafetyIdentifier = (
  email: string | null,
  ip: string | null
): string | undefined => {
  const identity = email?.trim()
    ? `email:${email.trim().toLowerCase()}`
    : ip?.trim()
    ? `ip:${ip.trim().toLowerCase()}`
    : null;

  if (!identity) {
    return undefined;
  }

  const config = getWibbleConfig();
  const secret =
    config.secrets.safety_identifier_secret.trim() ||
    config.secrets.nextauth_secret.trim();
  return secret
    ? createHmac("sha256", secret).update(identity).digest("hex")
    : createHash("sha256").update(identity).digest("hex");
};
