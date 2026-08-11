import { defaultHandler } from "ra-data-simple-prisma";
import { NextApiRequest, NextApiResponse } from "next";
import { requireSession } from "@/core/serverSession";
import prisma from "@/core/PrismaWibble";
import {
  deleteContentWithRelations,
  deleteManyContentWithRelations,
} from "@/core/adminContentDeletion";

export const config = {
  api: {
    responseLimit: false,
  },
};

const handler = async (req: NextApiRequest, res: NextApiResponse) => {
  if (!(await requireSession(req, res))) return;

  if (req.query.resource === "content" && req.body?.method === "delete") {
    const id = req.body?.params?.id;
    if (typeof id !== "string" || !id) {
      res.status(400).json({ message: "Missing article id" });
      return;
    }

    const deleted = await prisma.$transaction((transaction) =>
      deleteContentWithRelations(transaction, id)
    );
    res.status(200).json({ data: deleted });
    return;
  }

  if (req.query.resource === "content" && req.body?.method === "deleteMany") {
    const ids = req.body?.params?.ids;
    if (
      !Array.isArray(ids) ||
      ids.length === 0 ||
      ids.some((id) => typeof id !== "string" || !id)
    ) {
      res.status(400).json({ message: "Missing article ids" });
      return;
    }

    const deletedIds = await prisma.$transaction((transaction) =>
      deleteManyContentWithRelations(transaction, ids)
    );
    res.status(200).json({ data: deletedIds });
    return;
  }

  await defaultHandler(req, res, prisma);
};

export default handler;
