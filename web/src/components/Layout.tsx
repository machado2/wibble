import { ReactNode } from "react";
import { Row, Col } from "antd";
import styles from "./Layout.module.css";
import Link from "next/link";
import {
  FaDiscord,
  FaHatWizard,
  FaPlusCircle,
  FaRss,
  FaSignOutAlt,
  FaUserCircle,
} from "react-icons/fa";
import { signIn, signOut, useSession } from "next-auth/react";
import { isAdmin } from "@/core/isAdmin";
import { useRuntimeConfig } from "./useRuntimeConfig";
import { GlobalLanguageSelector } from "./GlobalLanguageSelector";
import { useGlobalLanguage } from "./GlobalLanguage";

type LayoutProps = {
  children: ReactNode;
};

function Layout({ children }: LayoutProps) {
  const { discordUrl, ssoUrl } = useRuntimeConfig();
  const { copy, language } = useGlobalLanguage();
  const { data: session } = useSession();
  const isDerp = isAdmin(session);
  const login = () => signIn("sso", { callbackUrl: window.location.href });
  const logout = async () => {
    await signOut({ redirect: false });
    const returnTo = `${window.location.origin}/`;
    window.location.assign(
      `${ssoUrl}/logout?return_to=${encodeURIComponent(returnTo)}`
    );
  };
  return (
    <>
      <Row className={styles.siteheader} align="middle">
        <Col flex="0 1 auto" style={{ textAlign: "left" }}>
          <Link
            href={discordUrl}
            target="_blank"
            rel="noreferrer"
            title="Discord"
          >
            <FaDiscord style={{ fontSize: "24px", cursor: "pointer" }} />
          </Link>
        </Col>
        <Col flex="1 1 auto" style={{ textAlign: "center" }}>
          <div className={styles.titleCluster}>
            <Link href={{ pathname: "/", query: language ? { lang: language } : {} }} title={copy.home}>
              <h1>The Wibble</h1>
            </Link>
            <GlobalLanguageSelector />
          </div>
        </Col>
        <Col
          flex="0 1 auto"
          style={{ textAlign: "right", paddingRight: "16px" }}
        >
          <Link href="/create" title={copy.generateArticle}>
            <FaPlusCircle style={{ fontSize: "24px" }} />
          </Link>
        </Col>
        {isDerp && (
          <Col
            flex="0 1 auto"
            style={{ textAlign: "right", paddingRight: "16px" }}
          >
            <Link href="/derpmin" target="_blank" title="Derp">
              <FaHatWizard style={{ fontSize: "24px" }} />
            </Link>
          </Col>
        )}
        <Col
          flex="0 1 auto"
          style={{ textAlign: "right", paddingRight: "16px" }}
        >
          <Link href="/rss.xml" target="_blank" title={copy.rssFeed}>
            <FaRss style={{ fontSize: "24px" }} />
          </Link>
        </Col>
        <Col flex="0 1 auto">
          <div className={styles.signup}>
            {session && session.user ? (
              <button
                className={styles.authButton}
                onClick={() => void logout()}
              >
                <FaSignOutAlt style={{ fontSize: "24px" }} title={copy.signOut} />
              </button>
            ) : (
              <button
                className={styles.authButton}
                onClick={() => void login()}
              >
                <FaUserCircle style={{ fontSize: "24px" }} title={copy.signIn} />
              </button>
            )}
          </div>
        </Col>
      </Row>
      <div className={styles.content}>{children}</div>
    </>
  );
}

export default Layout;
