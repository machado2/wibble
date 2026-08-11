import { useEffect, useState } from "react";
import { useRouter } from "next/router";
import axios from "axios";
import { Button, Card, Input, Spin, message } from "antd";
import Head from "next/head";
import styles from "@/styles/create.module.css";
import { useSession } from "next-auth/react";
import { isAdmin } from "@/core/isAdmin";

const { TextArea } = Input;

const GenerateNewArticle = () => {
  const [prompt, setPrompt] = useState<string>("");
  const [targetUrl, setTargetUrl] = useState<string>("");
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const router = useRouter();
  const { data: session } = useSession();
  const admin = isAdmin(session);

  useEffect(() => {
    if (!admin || !router.isReady || typeof router.query.url !== "string") {
      return;
    }
    const requestedUrl = router.query.url;
    setTargetUrl((current) => current || requestedUrl);
  }, [admin, router.isReady, router.query.url]);

  const submit = async () => {
    const trimmedPrompt = prompt.trim();
    if (!trimmedPrompt) {
      message.error("Please enter a prompt.");
      return;
    }
    if (trimmedPrompt.length > 2000) {
      message.error("Your text is too long!");
      return;
    }

    setIsLoading(true);
    try {
      const response = await axios.post<{ slug: string }>("/api/create", {
        prompt: trimmedPrompt,
        url: admin && targetUrl.trim() ? targetUrl.trim() : undefined,
      });
      router.push(`/content/${response.data.slug}`);
    } catch (error: any) {
      message.error(error.response?.data?.error || "Something went wrong!");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <>
      <Head>
        <title>Generate New Article</title>
      </Head>
      <Card className="app-container" bordered={false}>
        <Spin spinning={isLoading}>
          <h1>{admin ? "Enter the target URL and prompt" : "Enter your prompt"}</h1>
          <p>
            Type anything here for guiding the AI and it will try to guess what
            you mean. You can try a title, part of a title, keywords,
            instructions, or anything else you can think of.
          </p>
          {admin && (
            <Input
              className={styles.inputfield}
              value={targetUrl}
              onChange={(e) => setTargetUrl(e.target.value)}
              placeholder="/content/article-slug"
              aria-label="Target article URL"
            />
          )}
          <TextArea
            className={styles.inputfield}
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="Enter your prompt / hint / instructions here..."
            rows={10}
            autoFocus
          />
          <Button
            onClick={submit}
            type="primary"
            size="large"
            style={{ marginTop: "20px" }}
            disabled={isLoading}
          >
            Submit
          </Button>
        </Spin>
      </Card>
    </>
  );
};

export default GenerateNewArticle;
