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
  const altText = props.alt;
  const cssClass = props.className;
  const [captionUuid] = useState<string>(uuidv4());
  const [mounted, setMounted] = useState(false);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const retryTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const imgid = computeHash(props.prompt);
  const imgUrl = `/api/image/${imgid}${
    retryAttempt > 0 ? `?retry=${retryAttempt}` : ""
  }`;
  const captionText = props.alt.length > 0 ? props.alt : props.prompt ?? "";

  useEffect(() => {
    setMounted(true);
    return () => {
      if (retryTimer.current) clearTimeout(retryTimer.current);
    };
  }, []);

  const retryPendingImage = () => {
    if (retryAttempt >= 24 || retryTimer.current) return;
    retryTimer.current = setTimeout(() => {
      retryTimer.current = null;
      setRetryAttempt((attempt) => attempt + 1);
    }, 5000);
  };

  const imageLoaded = () => {
    if (retryTimer.current) {
      clearTimeout(retryTimer.current);
      retryTimer.current = null;
    }
  };

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

  if (props.prompt && mounted) {
    const captionHtml = `<div id="${captionUuid}" class="${styles.promptcaption}" onpointermove="event.stopPropagation()">`;
    return (
      <>
        <Gallery withCaption>
          <Item
            original={imgUrl}
            thumbnail={imgUrl}
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
                  src={imgUrl}
                  className={cssClass}
                  onError={retryPendingImage}
                  onLoad={imageLoaded}
                />
              );
            }}
          </Item>
        </Gallery>
      </>
    );
  }

  return (
    <img
      src={imgUrl}
      alt={altText}
      title={captionText}
      className={cssClass}
      onError={retryPendingImage}
      onLoad={imageLoaded}
    />
  );
};
