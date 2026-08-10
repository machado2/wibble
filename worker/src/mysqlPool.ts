import mysql from "mysql2";
import { Config } from "./config";

const createMysqlPool = () => {
  const databaseUrl = Config.databaseUrl;
  const parsedUrl = new URL(databaseUrl);
  const username = parsedUrl.username;
  const password = parsedUrl.password;
  const database = parsedUrl.pathname.substring(1);
  const host = parsedUrl.hostname;
  const port = parsedUrl.port;
  return mysql.createPool({
    host,
    port: parseInt(port),
    user: username,
    password,
    database,
    waitForConnections: true,
    connectionLimit: 10,
    queueLimit: 0,
  });
};

const mysqlPool = createMysqlPool();

export default mysqlPool;
