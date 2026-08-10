// pages/admin.tsx
import React from 'react';
import dynamic from 'next/dynamic';
import { NextPage } from 'next';
const DynamicAdminPanel = dynamic(() => import('@/admin/AdminPanel').then((mod) => mod.AdminPanel), { ssr: false });


const AdminPage: NextPage<{ hideHeader: boolean }> = () => {
  return <DynamicAdminPanel />;
};

AdminPage.getInitialProps = async () => {
  return { hideHeader: true };
};

export default AdminPage;
