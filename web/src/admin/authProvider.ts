// src/admin/authProvider.ts
import { AuthProvider } from "react-admin";
import { signIn, signOut, getSession } from "next-auth/react";
import {
  defaultPublicRuntimeConfig,
  type PublicRuntimeConfig,
} from "@/core/runtimeConfig";

export const authProvider: AuthProvider = {
  login: async ({ username, password }) => {
    void username;
    void password;
    await signIn("sso", { callbackUrl: "/derpmin" });
  },
  logout: async () => {
    await signOut({ redirect: false });
    const runtimeConfig = await fetch("/api/runtime-config")
      .then((response) =>
        response.ok
          ? (response.json() as Promise<PublicRuntimeConfig>)
          : defaultPublicRuntimeConfig
      )
      .catch(() => defaultPublicRuntimeConfig);
    window.location.assign(
      `${runtimeConfig.ssoUrl}/logout?return_to=${encodeURIComponent(
        `${window.location.origin}/`
      )}`
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
