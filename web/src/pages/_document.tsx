import { Html, Head, Main, NextScript } from "next/document";

export default function Document() {
  return (
    <Html lang="en">
      <Head>
        <link rel="icon" href="/wibble2.jpeg" />
        <link
          rel="alternate"
          type="application/rss+xml"
          href="https://wibble.news/rss.xml"
          title="The Wibble RSS Feed"
        />
        <script
          async
          src="https://umami.wibble.news/script.js"
          data-website-id="c17df13e-fe8d-4088-a556-c38d71b3dbb6"
        ></script>
      </Head>
      <body>
        <Main />
        <NextScript />
      </body>
    </Html>
  );
}
