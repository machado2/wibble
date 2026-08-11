import { NextAuthOptions, Session, User } from "next-auth";
import type { OAuthConfig } from "next-auth/providers";
import { getWibbleConfig } from "../../../config-runtime";

export type UserWithRole = User & { role: string };

type SsoProfile = {
  sub: string;
  name?: string;
  email?: string;
  picture?: string;
};

export const authOptions = (): NextAuthOptions => {
  const config = getWibbleConfig();
  const ssoIssuer = config.auth.sso_issuer_url.replace(/\/$/, "");
  const ssoProvider: OAuthConfig<SsoProfile> = {
    id: "sso",
    name: "FBMac SSO",
    type: "oauth",
    wellKnown: `${ssoIssuer}/.well-known/openid-configuration`,
    clientId: config.auth.sso_client_id,
    clientSecret: config.secrets.sso_client_secret,
    authorization: {
      params: { scope: "openid profile email" },
    },
    client: {
      id_token_signed_response_alg: "EdDSA",
    },
    userinfo: {
      async request({ tokens, client }) {
        if (!tokens.access_token) {
          throw new Error("SSO did not return an access token");
        }
        return client.userinfo(tokens.access_token);
      },
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

  return {
    providers: [ssoProvider],
    secret: config.secrets.nextauth_secret,
    useSecureCookies: config.app.site_url.startsWith("https://"),
    callbacks: {
      async session(params) {
        const session = params.session as Session;
        const user = session?.user as UserWithRole | undefined;
        const adminEmail = config.secrets.admin_email;
        if (user && user.email === adminEmail) {
          user.role = "admin";
        }
        return session;
      },
    },
  };
};
