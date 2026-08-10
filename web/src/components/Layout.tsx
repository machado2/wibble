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
import { useSession } from "next-auth/react";
import { isAdmin } from "@/core/isAdmin";

type LayoutProps = {
  children: ReactNode;
};

function Layout({ children }: LayoutProps) {
  const discordServerURL = "https://discord.gg/qwATcUrFe";
  const { data: session } = useSession();
  const isDerp = isAdmin(session);
  return (
    <>
      <Row className={styles.siteheader} align="middle">
        <Col flex="0 1 auto" style={{ textAlign: "left" }}>
          <Link
            href={discordServerURL}
            target="_blank"
            rel="noreferrer"
            title="Discord"
          >
            <FaDiscord style={{ fontSize: "24px", cursor: "pointer" }} />
          </Link>
        </Col>
        <Col flex="1 1 auto" style={{ textAlign: "center" }}>
          <Link href="/" title="Home">
            <h1>The Wibble</h1>
          </Link>
        </Col>
        <Col
          flex="0 1 auto"
          style={{ textAlign: "right", paddingRight: "16px" }}
        >
          <Link href="/create" title="Generate new article">
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
          <Link href="/rss.xml" target="_blank" title="RSS Feed">
            <FaRss style={{ fontSize: "24px" }} />
          </Link>
        </Col>
        <Col flex="0 1 auto">
          <div className={styles.signup}>
            {session && session.user ? (
              <Link href="/api/auth/signout">
                <FaSignOutAlt style={{ fontSize: "24px" }} title="Sign-out" />
              </Link>
            ) : (
              <Link href="/api/auth/signin">
                <FaUserCircle style={{ fontSize: "24px" }} title="Sign-in" />
              </Link>
            )}
          </div>
        </Col>
      </Row>
      <div className={styles.content}>{children}</div>
    </>
  );
}

export default Layout;
