import { PrismaClient } from "@prisma/client";
import { getWibbleConfig } from "../../../config-runtime";

let activeClient: PrismaClient | null = null;
let activeDatabaseUrl = "";

const currentClient = (): PrismaClient => {
  const databaseUrl = getWibbleConfig().secrets.database_url.trim();
  if (!databaseUrl) {
    throw new Error("Missing Nickel configuration secrets.database_url");
  }
  if (activeClient && activeDatabaseUrl === databaseUrl) {
    return activeClient;
  }

  const previous = activeClient;
  activeClient = new PrismaClient({
    datasources: { db: { url: databaseUrl } },
  });
  activeDatabaseUrl = databaseUrl;
  if (previous) {
    setTimeout(() => void previous.$disconnect(), 30_000).unref();
  }
  return activeClient;
};

const prisma = new Proxy({} as PrismaClient, {
  get(_target, property) {
    const client = currentClient();
    const value = Reflect.get(client, property, client);
    return typeof value === "function" ? value.bind(client) : value;
  },
});

export default prisma;
