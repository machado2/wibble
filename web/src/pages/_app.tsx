import '@/styles/globals.css'
import type { AppProps } from 'next/app'
import Layout from '@/components/Layout'
import { SessionProvider } from "next-auth/react"
import { MDXProvider } from '@mdx-js/react'

import { GlobalLanguageProvider } from '@/components/GlobalLanguage'

export default function App({ Component, pageProps: { session, ...pageProps } }: AppProps) {
  return (
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
  );
}
