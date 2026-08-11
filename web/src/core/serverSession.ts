import { NextApiRequest, NextApiResponse } from "next";
import { getToken } from "next-auth/jwt";
import { getWibbleConfig } from "../../../config-runtime";

const getServerToken = async (req: NextApiRequest) => {
  const config = getWibbleConfig();
  return getToken({
    req,
    secret: config.secrets.nextauth_secret,
    secureCookie: config.app.site_url.startsWith("https://"),
  });
};

export const getServerEmail = async (
  req: NextApiRequest,
  _res: NextApiResponse
) => {
  const token = await getServerToken(req);
  return typeof token?.email === "string" ? token.email : null;
};

export const isServerAdmin = (email: string | null): boolean => {
  const adminEmail = getWibbleConfig().secrets.admin_email;
  return Boolean(email && adminEmail && email === adminEmail);
};

export const requireSession = async (
  req: NextApiRequest,
  res: NextApiResponse
): Promise<boolean> => {
  const email = await getServerEmail(req, res);
  if (!isServerAdmin(email)) {
    res.status(401).json({ message: "Unauthorized" });
    return false;
  }
  return true;
};
