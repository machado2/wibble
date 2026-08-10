import { defaultHandler } from "ra-data-simple-prisma";
import { NextApiRequest, NextApiResponse } from "next";
import { requireSession } from "@/core/serverSession";
import prisma from "@/core/PrismaWibble";
import { image_cache } from "@prisma/client";

export const config = {
  api: {
    responseLimit: false,
  },
};

const handler = async (req: NextApiRequest, res: NextApiResponse) => {
  await requireSession(req, res);
  await defaultHandler(req, res, prisma, {
    getList: {
        transform: (list: image_cache[]) => list,
    },
    getOne: {
      transform: (row: image_cache) => row,
    },
  });
};

export default handler;
