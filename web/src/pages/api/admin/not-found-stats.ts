import type { NextApiRequest, NextApiResponse } from "next";
import prisma from "@/core/PrismaWibble";
import { requireSession } from "@/core/serverSession";

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse,
) {
  if (!(await requireSession(req, res))) return;
  if (req.method !== "GET") {
    res.status(405).json({ error: "Method not allowed" });
    return;
  }

  const [uniqueUrls, unresolvedUrls, totals] = await Promise.all([
    prisma.not_found_request.count(),
    prisma.not_found_request.count({ where: { generated_at: null } }),
    prisma.not_found_request.aggregate({ _sum: { hit_count: true } }),
  ]);

  res.status(200).json({
    uniqueUrls,
    unresolvedUrls,
    totalHits: totals._sum.hit_count ?? 0,
    generatedUrls: uniqueUrls - unresolvedUrls,
  });
}
