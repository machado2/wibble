import { useState, useEffect } from "react";
import styles from "./ImageList.module.css";
import Link from "next/link";
import { useRouter } from "next/router";
import { dontWaitFor } from "@/core/dontWaitFor";
import { ImageListItem } from "@/pages/api/images";
import { ImageListItemUI } from "./ImageListItemUI";
import { useGlobalLanguage } from "./GlobalLanguage";

const PAGE_SIZE = 30;

export default function ImageList() {
  const router = useRouter();
  const { copy } = useGlobalLanguage();
  const [images, setImages] = useState<ImageListItem[] | undefined>(undefined);
  const [error, setError] = useState("");

  const readString = (p: string): string | undefined => {
    const str = router.query[p];
    if (str === undefined) return undefined;
    if (typeof str === "string") return str;
    return str[0];
  };

  const searchTerm = readString("search");
  const model = readString("model");
  const afterId = readString("afterId");
  const t = readString("t");
  const sort = readString("sort");

  useEffect(() => {
    const load = async () => {
      setImages(undefined);
      setError("");
      const queryParams = new URLSearchParams();
      queryParams.set("pageSize", PAGE_SIZE.toString());
      if (searchTerm && searchTerm.length > 0) {
        queryParams.set("search", searchTerm);
      }
      if (model && model.length > 0) {
        queryParams.set("model", model);
      }
      if (t) {
        queryParams.set("t", t);
      }
      if (afterId) {
        queryParams.set("afterId", afterId);
      }
      if (sort) {
        queryParams.set("sort", sort);
      }
      try {
        const response = await fetch(`/api/images?${queryParams}`, {
          method: "GET",
        });
        if (!response.ok) {
          throw new Error(response.statusText);
        }
        setImages((await response.json()) as ImageListItem[]);
      } catch (loadError) {
        console.error(loadError);
        setImages([]);
        setError(copy.imagesLoadError);
      }
    };

    dontWaitFor(load());
  }, [copy.imagesLoadError, searchTerm, model, afterId, t, sort]);

  if (!images) {
    return <p>{copy.loadingImages}</p>;
  }

  return (
    <>
      {error ? <p role="alert">{error}</p> : null}
      {!error && images.length === 0 ? <p>{copy.noImages}</p> : null}
      <div className={styles.ImageListContainer}>
        {images.map((item) => (
          <div key={`${item.id}`} className={styles.ImageListItem}>
            <ImageListItemUI item={item} key={item.id} />
          </div>
        ))}
      </div>
      <div className={styles.listBottom}></div>
      {images.length === PAGE_SIZE ? (
        <Link
          className={styles.nextPageLink}
          href={{
            pathname: router.pathname,
            query: { ...router.query, afterId: images[images.length - 1].id },
          }}
        >
          {copy.nextPage}
        </Link>
      ) : null}
    </>
  );
}
