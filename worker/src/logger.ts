import winston from "winston";
import { Loggly } from "winston-loggly-bulk";
import { Config } from "./config";

if (Config.logglyToken.length > 0) {
  winston.add(
    new Loggly({
      token: Config.logglyToken,
      subdomain: "fbmac",
      tags: ["wibble-worker"],
      json: true,
    })
  );
}

winston.add(
  new winston.transports.Console({
    format: winston.format.simple(),
  })
);

export default winston;
