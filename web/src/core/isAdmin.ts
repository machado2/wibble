import type { Session } from "next-auth";
import type { UserWithRole } from "./authOptions";

export const isAdmin = (session: Session | null | undefined): boolean => {
  const user = session?.user as UserWithRole | undefined;
  return (user && user.role === "admin") || false;
};
