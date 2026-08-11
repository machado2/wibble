import winston from "winston";
import { Loggly } from "winston-loggly-bulk";
import { getWibbleConfig } from "../../../config-runtime";

let activeLogger: winston.Logger | null = null;
let activeSignature = "";

const currentLogger = (): winston.Logger => {
  const config = getWibbleConfig();
  const token = config.secrets.loggly_token.trim();
  const signature = JSON.stringify([
    token,
    config.logging.loggly_subdomain,
    config.logging.web_tags,
  ]);
  if (activeLogger && activeSignature === signature) {
    return activeLogger;
  }

  const next = winston.createLogger({
    transports: [
      new winston.transports.Console({ format: winston.format.simple() }),
    ],
  });
  if (token) {
    next.add(
      new Loggly({
        token,
        subdomain: config.logging.loggly_subdomain,
        tags: config.logging.web_tags,
        json: true,
      })
    );
  }

  const previous = activeLogger;
  activeLogger = next;
  activeSignature = signature;
  previous?.close();
  return next;
};

const logger = {
  info: (message: string, ...meta: unknown[]) =>
    currentLogger().info(message, ...meta),
  warn: (message: string, ...meta: unknown[]) =>
    currentLogger().warn(message, ...meta),
  error: (message: string, ...meta: unknown[]) =>
    currentLogger().error(message, ...meta),
  debug: (message: string, ...meta: unknown[]) =>
    currentLogger().debug(message, ...meta),
};

export default logger;
