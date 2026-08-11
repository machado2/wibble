import { authOptions } from "@/core/authOptions";
import NextAuth from "next-auth";
import type { NextApiRequest, NextApiResponse } from "next";

export default function handler(req: NextApiRequest, res: NextApiResponse) {
  return NextAuth(req, res, authOptions());
}
