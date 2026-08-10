import { useState } from "react";
import { useRouter } from "next/router";
import axios from "axios";
import { Button, Card, Input, Spin, message } from "antd";
import Head from "next/head";
import styles from "@/styles/create.module.css";
import { useSession } from "next-auth/react";
import { isAdmin } from "@/core/isAdmin";
import { ModelSelection } from "@/components/ModelSelection";

const { TextArea } = Input;

const GenerateNewArticle = () => {
  const [prompt, setPrompt] = useState<string>("");
  const [model, setModel] = useState<string | undefined>(undefined);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const router = useRouter();
  const { data: session } = useSession();
  const admin = isAdmin(session);

  const submit = async () => {
    if (prompt.length > 2000) {
      message.error("Your text is too long!");
      return;
    }

    setIsLoading(true);
    try {
      const response = await axios.post<{ slug: string }>("/api/create", {
        prompt,
        model,
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
          <h1>Enter your prompt</h1>
          <p>
            Type anything here for guiding the AI and it will try to guess what
            you mean. You can try a title, part of a title, keywords,
            instructions, or anything else you can think of.
          </p>
          <TextArea
            className={styles.inputfield}
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="Enter your prompt / hint / instructions here..."
            rows={10}
            autoFocus
          />
          {admin && <ModelSelection value={model} onChange={setModel} />}
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
