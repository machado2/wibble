import mysqlPool from "./mysqlPool";
import { RateLimiterMySQL } from "rate-limiter-flexible";

export const getRateLimiter = (
  points: number,
  duration: number,
  keyPrefix?: string
) =>
  new Promise<RateLimiterMySQL>((resolve, reject) => {
    let rateLimiterMySQL: null | RateLimiterMySQL = null;
    const databaseName = new URL(
      process.env.DATABASE_URL as string
    ).pathname.slice(1);

    const ready = (err: any) => {
      if (err) {
        reject(err);
      } else {
        resolve(rateLimiterMySQL!);
      }
    };
    rateLimiterMySQL = new RateLimiterMySQL(
      {
        storeClient: mysqlPool,
        points,
        duration,
        keyPrefix,
        dbName: databaseName,
      },
      ready
    );
  });
