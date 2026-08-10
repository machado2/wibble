import React, { useEffect, useRef, useState } from "react";
import { Gallery, Item } from "react-photoswipe-gallery";
import "photoswipe/dist/photoswipe.css";
import styles from "@/styles/GeneratedImage.module.css";
import { v4 as uuidv4 } from "uuid";
import { ImageCaption } from "./ImageCaption/ImageCaption";
import { createRoot } from "react-dom/client";
import { computeHash } from "@/core/computeHash";

export type BoxedImageProps = {
  alt: string;
  className: string | undefined;
  prompt: string;
};

export const BoxedImage = (props: BoxedImageProps) => {
  const cssClass = props.className;
  const [captionUuid] = useState<string>(uuidv4());
  const [mounted, setMounted] = useState(false);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const [loadedImageUrl, setLoadedImageUrl] = useState<string | null>(null);
  const retryTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const imgid = computeHash(props.prompt);
  const imgUrl = `/api/image/${imgid}${
    retryAttempt > 0 ? `?retry=${retryAttempt}` : ""
  }`;
  const captionText = props.alt.length > 0 ? props.alt : props.prompt ?? "";

  useEffect(() => {
    setMounted(true);
  }, []);

  useEffect(() => {
    if (!mounted || !props.prompt) return;

    const controller = new AbortController();
    let objectUrl: string | null = null;
    setLoadedImageUrl(null);

    const loadImage = async () => {
      try {
        const response = await fetch(imgUrl, {
          cache: "no-store",
          signal: controller.signal,
        });
        if (response.ok) {
          objectUrl = URL.createObjectURL(await response.blob());
          setLoadedImageUrl(objectUrl);
          return;
        }
        if (response.status !== 503 || retryAttempt >= 24) return;
      } catch (error) {
        if (controller.signal.aborted || retryAttempt >= 24) return;
      }

      retryTimer.current = setTimeout(() => {
        retryTimer.current = null;
        setRetryAttempt((attempt) => attempt + 1);
      }, 5000);
    };

    void loadImage();
    return () => {
      controller.abort();
      if (retryTimer.current) {
        clearTimeout(retryTimer.current);
        retryTimer.current = null;
      }
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [imgUrl, mounted, props.prompt, retryAttempt]);

  const tryPlugCaption = () => {
    const elCaption = document.getElementById(captionUuid);
    if (elCaption) {
      if (elCaption.childNodes.length > 0) {
        return;
      }
      const root = createRoot(elCaption!);
      root.render(
        <ImageCaption imgid={imgid!} alt={captionText} prompt={props.prompt!} />
      );
    }
  };

  if (props.prompt && mounted && loadedImageUrl) {
    const captionHtml = `<div id="${captionUuid}" class="${styles.promptcaption}" onpointermove="event.stopPropagation()">`;
    return (
      <>
        <Gallery withCaption>
          <Item
            original={loadedImageUrl}
            thumbnail={loadedImageUrl}
            width="512"
            height="512"
            caption={captionHtml}
          >
            {({ ref, open }) => {
              const openLightbox = async (
                e: React.MouseEvent<Element, MouseEvent>
              ) => {
                try {
                  open(e);
                  tryPlugCaption();
                } catch (e) {}
              };
              return (
                <img
                  ref={ref as any}
                  onClick={openLightbox}
                  alt={captionText}
                  title={captionText}
                  src={loadedImageUrl}
                  className={cssClass}
                  onError={() => setLoadedImageUrl(null)}
                />
              );
            }}
          </Item>
        </Gallery>
      </>
    );
  }

  return null;
};
