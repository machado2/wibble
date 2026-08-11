import { useRouter } from "next/router";
import { useState, useEffect } from "react";
import { Spin, Card, Alert } from "antd";
import Head from "next/head";
import React from "react";
import { MDXRemote } from "next-mdx-remote";
import { ContentService, ParsedResponse } from "@/core/ContentService";
import {
  GetServerSidePropsContext,
  NextApiRequest,
  NextApiResponse,
} from "next";
import { GeneratedImage } from "@/components/GeneratedImage";
import styles from "@/styles/Slug.module.css";
import { getServerEmail } from "@/core/serverSession";
import { SupportTheWibble } from "@/components/SupportTheWibble";
import { VoteButtons } from "@/components/VoteButtons";
import { getHumanReadableDate } from "@/core/getHumanReadableDate";
import { NotFoundRepository } from "@/core/NotFoundRepository";
import { articleTargetForSlug } from "@/core/articleTarget";
import { getWibbleConfig } from "../../../../config-runtime";

const defaultDescription = `Get the latest news with a touch of wobble from The Wibble,
your source for the unpredictable and unsteady world of current events.`;
const defaultTitle = "The Wibble";

async function tryLoadContent(slug: string): Promise<ParsedResponse> {
  const response = await fetch(`/api/content/${slug}`);
  if (!response.ok) {
    throw new Error(response.statusText);
  }
  const data = await response.json();
  return data as ParsedResponse;
}

const mdxComponents = {
  GeneratedImage,
};

function SlugPage(data: ParsedResponse & { publicSiteUrl: string }) {
  const router = useRouter();
  const slug: string = router.query.slug as string;
  const [parsedResponse, setParsedResponse] = useState<ParsedResponse>(data);
  const [error, setError] = useState(false);

  useEffect(() => {
    let cancelled = false;
    if (slug) {
      const fetchData = async () => {
        try {
          let keepTrying = parsedResponse.loading;
          while (keepTrying) {
            await new Promise((resolve) => setTimeout(resolve, 5000));
            const newResponse = await tryLoadContent(slug);
            if (cancelled) return;
            setParsedResponse(newResponse);
            keepTrying = newResponse.loading;
          }
        } catch (error) {
          console.log(error);
          setError(true);
        }
      };
      fetchData();
    }
    return () => {
      cancelled = true;
    };
  }, [parsedResponse.loading, slug]);

  const title = (parsedResponse?.content?.frontmatter.title ||
    defaultTitle) as string;
  const displayTitle = error
    ? "Error"
    : `${parsedResponse?.loading ? "[working...] " : ""}${title}`;
  const description = (parsedResponse?.content?.frontmatter.description ||
    defaultDescription) as string;
  const addTitle = parsedResponse.titleInContent !== true;

  return (
    <>
      <Head>
        <title>{displayTitle}</title>
        <meta property="og:title" content={title} />
        <meta property="og:description" content={description} />
        <meta
          property="og:image"
          content={`${data.publicSiteUrl}${
            parsedResponse?.imageUrl || "/wibble2.jpeg"
          }`}
        />
      </Head>
      <Card className={styles.article} bordered={false}>
        {slug ? (
          <Spin spinning={parsedResponse?.loading && !error}>
            {addTitle ? <h1>{title}</h1> : null}
            {parsedResponse?.datetime ? (
              <p className={styles.datetime}>
                {getHumanReadableDate(parsedResponse?.datetime)}
              </p>
            ) : null}
            {error ? (
              <Alert message="Failed to load content." type="error" showIcon />
            ) : (
              <MDXRemote
                components={mdxComponents}
                {...parsedResponse?.content}
              />
            )}
          </Spin>
        ) : null}
        {!parsedResponse.loading && (
          <VoteButtons
            votes={parsedResponse?.votes ?? 0}
            contentId={parsedResponse?.id}
            currentVote={parsedResponse?.currentVote}
          />
        )}
        <SupportTheWibble />
      </Card>
    </>
  );
}

export async function getServerSideProps(context: GetServerSidePropsContext) {
  const { slug } = context.query;
  if (!slug || Array.isArray(slug)) {
    return { notFound: true };
  }

  const service = new ContentService();
  const siteUrl = getWibbleConfig().app.site_url;
  try {
    const email = await getServerEmail(
      context.req as NextApiRequest,
      context.res as NextApiResponse
    );
    const data = await service.processSlug(email, slug as string);
    if (!data) {
      try {
        const target = articleTargetForSlug(slug, siteUrl);
        await new NotFoundRepository().record(target.url);
      } catch (recordError) {
        console.error("Failed to record missing article URL", recordError);
      }
      return { notFound: true };
    }
    return {
      props: {
        ...data,
        publicSiteUrl: siteUrl.replace(/\/$/, ""),
      },
    };
  } catch (error) {
    console.log(error);
    throw error;
  }
}

export default SlugPage;
