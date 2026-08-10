import { NextApiRequest, NextApiResponse } from "next";
import { getServerSession } from "next-auth/next";
import { isAdmin } from "./isAdmin";
import { authOptions } from "./authOptions";
import { HttpError } from "react-admin";

export const getServerSessionWibble = async (
  req: NextApiRequest,
  res: NextApiResponse
) => {
  return await getServerSession(req, res, authOptions);
};

export const getServerEmail = async (
  req: NextApiRequest,
  res: NextApiResponse
) => {
  const session = await getServerSessionWibble(req, res);
  return session?.user?.email ?? null;
};

export const requireSession = async (
  req: NextApiRequest,
  res: NextApiResponse
): Promise<void> => {
  const session = await getServerSessionWibble(req, res);
  if (!isAdmin(session)) {
    throw new HttpError(401, "Unauthorized");
  }
};
