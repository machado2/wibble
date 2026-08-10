import { Html, Head, Main, NextScript } from "next/document";

export default function Document() {
  return (
    <Html lang="en">
      <Head>
        <link rel="icon" href="/wibble2.jpeg" />
        <link
          rel="alternate"
          type="application/rss+xml"
          href="https://wibble.fbmac.net/rss.xml"
          title="The Wibble RSS Feed"
        />
      </Head>
      <body>
        <Main />
        <NextScript />
      </body>
    </Html>
  );
}
