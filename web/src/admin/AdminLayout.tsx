import React from "react";
import {
  AppBar,
  AppBarProps,
  DashboardMenuItem,
  Layout,
  LayoutProps,
  Logout,
  Menu,
  MenuItemLink,
  UserMenu,
} from "react-admin";
import {
  FaChartPie,
  FaExclamationTriangle,
  FaExternalLinkAlt,
  FaHatWizard,
  FaImages,
  FaNewspaper,
  FaRoute,
  FaSearch,
} from "react-icons/fa";
import styles from "./Admin.module.css";

const AdminUserMenu = () => (
  <UserMenu>
    <Logout />
  </UserMenu>
);

const WibbleAppBar = (props: AppBarProps) => (
  <AppBar {...props} className={styles.appBar} userMenu={<AdminUserMenu />}>
    <div className={styles.brand}>
      <span className={styles.brandMark}>
        <FaHatWizard />
      </span>
      <span className={styles.brandCopy}>
        <strong>Wibble Control</strong>
        <small>Editorial desk</small>
      </span>
    </div>
    <a className={styles.publicLink} href="/" target="_blank" rel="noreferrer">
      Ver site <FaExternalLinkAlt />
    </a>
  </AppBar>
);

const WibbleMenu = () => (
  <div className={styles.menuShell}>
    <div className={styles.menuIntro}>Operação</div>
    <Menu>
      <DashboardMenuItem
        primaryText="Visão geral"
        leftIcon={<FaChartPie />}
      />
      <MenuItemLink
        to="/content"
        primaryText="Artigos"
        leftIcon={<FaNewspaper />}
      />
      <MenuItemLink
        to="/image_cache"
        primaryText="Imagens"
        leftIcon={<FaImages />}
      />
      <MenuItemLink
        to="/not_found_request"
        primaryText="Rotas 404"
        leftIcon={<FaRoute />}
      />
      <MenuItemLink
        to="/search_history"
        primaryText="Buscas"
        leftIcon={<FaSearch />}
      />
      <MenuItemLink
        to="/history_generation_fail"
        primaryText="Falhas"
        leftIcon={<FaExclamationTriangle />}
      />
    </Menu>
  </div>
);

export const AdminLayout = (props: LayoutProps) => (
  <Layout {...props} appBar={WibbleAppBar} menu={WibbleMenu} />
);
