import '@/styles/globals.css'
import type { AppProps } from 'next/app'
import Layout from '@/components/Layout'
import { SessionProvider } from "next-auth/react"
import { MDXProvider } from '@mdx-js/react'
import Head from 'next/head'

import { GlobalLanguageProvider } from '@/components/GlobalLanguage'

export default function App({ Component, pageProps: { session, ...pageProps } }: AppProps) {
  return (
    <>
      <Head>
        <meta name="viewport" content="width=device-width, initial-scale=1" />
      </Head>
      <SessionProvider session={session}>
        <GlobalLanguageProvider>
          <MDXProvider>
            {pageProps.hideHeader ? (
              <Component {...pageProps} />
            ) : (
              <Layout>
                <Component {...pageProps} />
              </Layout>
            )}
          </MDXProvider>
        </GlobalLanguageProvider>
      </SessionProvider>
    </>
  );
}
