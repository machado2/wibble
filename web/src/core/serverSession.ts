import { NextApiRequest, NextApiResponse } from "next";
import { getToken } from "next-auth/jwt";

const authSecret =
  process.env.NEXTAUTH_SECRET ?? process.env.SSO_CLIENT_SECRET ?? "";

const getServerToken = async (req: NextApiRequest) =>
  getToken({ req, secret: authSecret });

export const getServerEmail = async (
  req: NextApiRequest,
  _res: NextApiResponse
) => {
  const token = await getServerToken(req);
  return typeof token?.email === "string" ? token.email : null;
};

export const requireSession = async (
  req: NextApiRequest,
  res: NextApiResponse
): Promise<boolean> => {
  const email = await getServerEmail(req, res);
  const adminEmail =
    process.env.ADMIN_EMAIL ?? process.env.REACT_ADMIN_EMAIL ?? "";
  if (!email || email !== adminEmail) {
    res.status(401).json({ message: "Unauthorized" });
    return false;
  }
  return true;
};
