import { ImageInfoResponse } from "@/pages/api/imageinfo";
import React, { useEffect, useRef, useState } from "react";

import styles from "./ImageCaption.module.css";
import { useGlobalLanguage, type GlobalCopy } from "../GlobalLanguage";

export type ImageCaptionProps = {
  imgid: string;
  alt: string;
  prompt: string;
  copy?: GlobalCopy;
};

export const ImageCaption: React.FC<ImageCaptionProps> = (props) => {
  const globalLanguage = useGlobalLanguage();
  const copy = props.copy ?? globalLanguage.copy;
  const captionRef = useRef<HTMLDivElement>(null);
  const [hasLoadedInfo, setHasLoadedInfo] = useState(false);
  const [imageInfo, setImageInfo] = useState<ImageInfoResponse | null>(null);

  useEffect(() => {
    const captionElement = captionRef.current;
    const controller = new AbortController();

    if (!hasLoadedInfo && captionElement) {
      const observer = new IntersectionObserver(
        (entries) => {
          const [entry] = entries;
          if (entry.isIntersecting) {
            setHasLoadedInfo(true);
            observer.disconnect();
            void (async () => {
              try {
                const response = await fetch(`/api/imageinfo?id=${props.imgid}`, {
                  signal: controller.signal,
                });
                if (!response.ok) return;
                const info = await response.json();
                if (!controller.signal.aborted) setImageInfo(info);
              } catch {
                // The prompt passed by the article remains a readable fallback.
              }
            })();
          }
        },
        { root: null, rootMargin: "0px", threshold: 0.1 }
      );

      observer.observe(captionElement);

      return () => {
        controller.abort();
        observer.disconnect();
      };
    }
    return () => controller.abort();
  }, [hasLoadedInfo, props.imgid]);

  let prompt: string;
  let negativePrompt: string | undefined;
  const rawPrompt = imageInfo?.prompt || props.prompt;
  const parts = rawPrompt.split("###");
  if (parts.length === 2) {
    prompt = parts[0].trim();
    negativePrompt = parts[1].trim();
  } else {
    prompt = rawPrompt;
  }

  return (
    <div
      ref={captionRef}
      className={styles.promptcaption}
      onPointerMove={(e) => {
        e.stopPropagation();
      }}
    >
      <h3>{props.alt}</h3>
      <details>
        <summary>{copy.promptDetails}</summary>
        <p>{copy.promptMeaning}</p>
        <p><strong>Prompt:</strong> {prompt}</p>
        {negativePrompt ? (
          <>
            <p>{copy.negativePromptMeaning}</p>
            <p><strong>Negative prompt:</strong> {negativePrompt}</p>
          </>
        ) : null}
        {imageInfo?.model ? <p>{copy.model}: {imageInfo.model}</p> : null}
        {imageInfo?.seed !== undefined ? <p>{copy.seed}: {imageInfo.seed}</p> : null}
      </details>
    </div>
  );
};
