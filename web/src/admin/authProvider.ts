// src/admin/authProvider.ts
import { AuthProvider } from "react-admin";
import { signIn, signOut, getSession } from "next-auth/react";

export const authProvider: AuthProvider = {
  login: async ({ username, password }) => {
    void username;
    void password;
    await signIn("sso", { callbackUrl: "/derpmin" });
  },
  logout: async () => {
    await signOut({ redirect: false });
    const ssoUrl =
      process.env.NEXT_PUBLIC_SSO_URL ?? "https://sso.fbmac.net";
    window.location.assign(
      `${ssoUrl}/logout?return_to=${encodeURIComponent(`${window.location.origin}/`)}`,
    );
  },
  checkAuth: async () => {
    // Use next-auth to get the session
    const session = await getSession();
    if (!session) {
      throw new Error("No current user");
    }
  },
  checkError: (error) => {
    // No need to check errors for this example
    if (error?.error === "Unauthorized") {
      return Promise.reject();
    }
    return Promise.resolve();
  },
  getPermissions: () => {
    // No need to get permissions for this example
    return Promise.resolve("");
  },
};
