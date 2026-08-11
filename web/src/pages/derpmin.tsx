// pages/admin.tsx
import React from "react";
import dynamic from "next/dynamic";
import {
  GetServerSidePropsContext,
  NextApiRequest,
  NextApiResponse,
  NextPage,
} from "next";
import { getServerEmail } from "@/core/serverSession";
import { getWibbleConfig } from "../../../config-runtime";
const DynamicAdminPanel = dynamic(
  () => import("@/admin/AdminPanel").then((mod) => mod.AdminPanel),
  { ssr: false }
);

const AdminPage: NextPage<{ hideHeader: boolean }> = () => {
  return <DynamicAdminPanel />;
};

export async function getServerSideProps(context: GetServerSidePropsContext) {
  const email = await getServerEmail(
    context.req as NextApiRequest,
    context.res as NextApiResponse
  );
  const adminEmail = getWibbleConfig().secrets.admin_email;

  if (!email) {
    return {
      redirect: {
        destination: "/api/auth/signin?callbackUrl=/derpmin",
        permanent: false,
      },
    };
  }
  if (!adminEmail || email !== adminEmail) {
    return { notFound: true };
  }
  return { props: { hideHeader: true } };
}

export default AdminPage;
