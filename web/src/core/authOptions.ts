import { NextAuthOptions, Session, User } from "next-auth";
import type { OAuthConfig } from "next-auth/providers";

export type UserWithRole = User & { role: string };

type SsoProfile = {
  sub: string;
  name?: string;
  email?: string;
  picture?: string;
};

const ssoIssuer = (
  process.env.SSO_ISSUER_URL ?? "https://sso.fbmac.net/api/auth"
).replace(/\/$/, "");

const ssoProvider: OAuthConfig<SsoProfile> = {
  id: "sso",
  name: "FBMac SSO",
  type: "oauth",
  wellKnown: `${ssoIssuer}/.well-known/openid-configuration`,
  clientId: process.env.SSO_CLIENT_ID ?? "",
  clientSecret: process.env.SSO_CLIENT_SECRET ?? "",
  authorization: {
    params: { scope: "openid profile email" },
  },
  checks: ["pkce", "state", "nonce"],
  profile(profile) {
    return {
      id: profile.sub,
      name: profile.name ?? profile.email ?? profile.sub,
      email: profile.email,
      image: profile.picture,
    };
  },
};

export const authOptions: NextAuthOptions = {
  providers: [ssoProvider],
  secret: process.env.NEXTAUTH_SECRET ?? process.env.SSO_CLIENT_SECRET,
  callbacks: {
    async session(params) {
      const session = params.session as Session;
      const user = session?.user as UserWithRole | undefined;
      const adminEmail =
        process.env.ADMIN_EMAIL ?? process.env.REACT_ADMIN_EMAIL ?? "";
      if (user && user.email === adminEmail) {
        user.role = "admin";
      }
      return session;
    },
  },
};
