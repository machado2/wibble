import Head from "next/head";
import Link from "next/link";

export default function NotFoundPage() {
  return (
    <main style={{ margin: "4rem auto", maxWidth: 640, textAlign: "center" }}>
      <Head>
        <title>Page not found - The Wibble</title>
      </Head>
      <h1>Page not found</h1>
      <p>The page you requested does not exist.</p>
      <Link href="/">Return to The Wibble</Link>
    </main>
  );
}
