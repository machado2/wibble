import winston from "winston";
import { Loggly } from "winston-loggly-bulk";

if (process.env.LOGGLY_TOKEN) {
  winston.add(
    new Loggly({
      token: process.env.LOGGLY_TOKEN,
      subdomain: "fbmac",
      tags: ["wibble-web"],
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
