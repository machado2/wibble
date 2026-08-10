import { defaultHandler } from "ra-data-simple-prisma";
import { NextApiRequest, NextApiResponse } from "next";
import { requireSession } from "@/core/serverSession";
import prisma from "@/core/PrismaWibble";

export const config = {
  api: {
    responseLimit: false,
  },
};

const handler = async (req: NextApiRequest, res: NextApiResponse) => {
  if (!(await requireSession(req, res))) return;
  await defaultHandler(req, res, prisma);
};

export default handler;
