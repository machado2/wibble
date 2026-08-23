import type { NextApiRequest, NextApiResponse } from "next";
import { requireSession } from "@/core/serverSession";
import {
  MAX_TRANSLATION_HOURLY_LIMIT,
  TranslationAdminService,
} from "@/core/TranslationAdminService";
import { getWibbleConfig } from "../../../../../config-runtime";

const service = new TranslationAdminService();

const requestOrigin = (req: NextApiRequest): string | null => {
  const origin = typeof req.headers.origin === "string" ? req.headers.origin : "";
  if (!origin) return null;
  try {
    const configured = new URL(getWibbleConfig().app.site_url).origin;
    return new URL(origin).origin === configured ? origin : null;
  } catch {
    return null;
  }
};

const requireSafeMutation = (req: NextApiRequest, res: NextApiResponse): boolean => {
  if (!requestOrigin(req)) {
    res.status(403).json({ message: "Origem da operação não autorizada." });
    return false;
  }
  if (
    (req.method === "PATCH" || req.method === "POST") &&
    !String(req.headers["content-type"] || "").toLowerCase().startsWith("application/json")
  ) {
    res.status(415).json({ message: "A operação exige JSON." });
    return false;
  }
  return true;
};

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (!(await requireSession(req, res))) return;

  try {
    if (req.method === "GET") {
      res.status(200).json(await service.snapshot());
      return;
    }

    if (!requireSafeMutation(req, res)) return;

    if (req.method === "PATCH") {
      const hourlyLimit = req.body?.hourlyLimit;
      if (
        !Number.isInteger(hourlyLimit) ||
        hourlyLimit < 0 ||
        hourlyLimit > MAX_TRANSLATION_HOURLY_LIMIT
      ) {
        res.status(400).json({
          message: `A quota deve ser um inteiro entre 0 e ${MAX_TRANSLATION_HOURLY_LIMIT}.`,
        });
        return;
      }
      res.status(200).json(await service.updateHourlyLimit(hourlyLimit));
      return;
    }

    if (req.method === "POST" && req.body?.action === "reset-budget") {
      res.status(200).json(await service.resetBudget());
      return;
    }

    if (req.method === "DELETE") {
      const id = typeof req.query.id === "string" ? req.query.id : "";
      if (!id) {
        res.status(400).json({ message: "ID da tarefa ausente." });
        return;
      }
      const result = await service.removeJob(id);
      if (result.removed) {
        res.status(200).json(result);
        return;
      }
      if (result.reason === "processing") {
        res.status(409).json({
          message: "A tarefa já está em execução e não pode ser removida com segurança.",
        });
        return;
      }
      res.status(404).json({ message: "Tarefa não encontrada." });
      return;
    }

    res.setHeader("Allow", "GET, PATCH, POST, DELETE");
    res.status(405).json({ message: "Método não permitido." });
  } catch (error) {
    console.error("Translation admin operation failed", error);
    res.status(500).json({ message: "A operação administrativa falhou." });
  }
}
