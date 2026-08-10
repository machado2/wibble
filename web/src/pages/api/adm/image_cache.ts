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
        transform: (list: image_cache[]) => {
            return list.map((row) => {
                return {
                    ...row,
                    image_data: undefined,
                };
            });
        },
    },
    getOne: {
      transform: (row: image_cache) => {
        return {
          ...row,
          image_data: undefined,
        };
      },
    },
  });
};

export default handler;
