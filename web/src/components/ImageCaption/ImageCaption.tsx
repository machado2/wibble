import { ImageInfoResponse } from "@/pages/api/imageinfo";
import React, { useEffect, useRef, useState } from "react";

import styles from "./ImageCaption.module.css";

export type ImageCaptionProps = {
  imgid: string;
  alt: string;
  prompt: string;
};

export const ImageCaption: React.FC<ImageCaptionProps> = (props) => {
  const captionRef = useRef<HTMLDivElement>(null);
  const [hasLoadedInfo, setHasLoadedInfo] = useState(false);
  const [imageInfo, setImageInfo] = useState<ImageInfoResponse | null>(null);

  useEffect(() => {
    const captionElement = captionRef.current;

    if (!hasLoadedInfo && captionElement) {
      const observer = new IntersectionObserver(
        async (entries) => {
          const [entry] = entries;
          if (entry.isIntersecting) {
            setHasLoadedInfo(true);
            observer.disconnect();

            const r = await fetch(`/api/imageinfo?id=${props.imgid}`);
            const info = await r.json();
            setImageInfo(info);
          }
        },
        { root: null, rootMargin: "0px", threshold: 0.1 }
      );

      observer.observe(captionElement);

      return () => {
        if (captionElement) {
          observer.unobserve(captionElement);
        }
      };
    }
  }, [hasLoadedInfo, props.imgid]);

  let prompt: string;
  let negativePrompt: string | undefined;
  if (imageInfo?.prompt) {
    const parts = imageInfo.prompt.split("###");
    if (parts.length === 2) {
      prompt = parts[0].trim();
      negativePrompt = parts[1].trim();
    } else {
      prompt = imageInfo.prompt;
    }
  } else {
    prompt = props.prompt;
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
        <summary>PROMPT DETAILS</summary>
        <p>PROMPT: {prompt}</p>
        {negativePrompt && <p>NEGATIVE PROMPT: {negativePrompt}</p>}
        <p>MODEL: {imageInfo?.model}</p>
        <p>SEED: {imageInfo?.seed}</p>
      </details>
    </div>
  );
};
