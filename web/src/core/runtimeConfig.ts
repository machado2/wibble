import type { PublicWriter } from "./writers";

export type PublicRuntimeConfig = {
  discordUrl: string;
  ssoUrl: string;
  maxPromptLength: number;
  writers: PublicWriter[];
};

export const defaultPublicRuntimeConfig: PublicRuntimeConfig = {
  discordUrl: "https://discord.gg/Ev2J94xDNY",
  ssoUrl: "https://sso.fbmac.net",
  maxPromptLength: 2000,
  writers: [],
};
