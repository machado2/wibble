import { getServerEmail, isServerAdmin } from "@/core/serverSession";
import type { PublicRuntimeConfig } from "@/core/runtimeConfig";
import { eligibleWriters } from "@/core/writers";
import { getWibbleConfig } from "../../../../config-runtime";
import type { NextApiRequest, NextApiResponse } from "next";

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse<PublicRuntimeConfig | { error: string }>
) {
  if (req.method !== "GET") {
    res.setHeader("Allow", "GET");
    res.status(405).json({ error: "Method not allowed" });
    return;
  }
  const email = await getServerEmail(req, res);
  const config = getWibbleConfig();
  res.setHeader("Cache-Control", "private, no-store");
  res.status(200).json({
    discordUrl: config.app.discord_url,
    ssoUrl: config.app.sso_public_url,
    maxPromptLength: config.generation.max_prompt_length,
    writers: eligibleWriters(
      config.generation.writers,
      isServerAdmin(email)
    ).map(({ id, nickname }) => ({ id, nickname })),
  });
}
