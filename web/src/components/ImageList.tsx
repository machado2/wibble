import { useState, useEffect } from "react";
import styles from "./ImageList.module.css";
import Link from "next/link";
import { useRouter } from "next/router";
import { dontWaitFor } from "@/core/dontWaitFor";
import { ImageListItem } from "@/pages/api/images";
import { ImageListItemUI } from "./ImageListItemUI";

export default function ImageList() {
  const router = useRouter();
  const [images, setImages] = useState<ImageListItem[]>([]);

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
      const queryParams = new URLSearchParams();
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
      const response = await fetch(`/api/images?${queryParams}`, {
        method: "GET",
      });
      if (!response.ok) {
        throw new Error(response.statusText);
      }
      setImages((await response.json()) as ImageListItem[]);
    };

    dontWaitFor(load());
  }, [searchTerm, model, afterId, t, sort]);

  if (!images) {
    return null;
  }

  return (
    <>
      <div className={styles.ImageListContainer}>
        {images.map((item) => (
          <div key={`${item.id}`} className={styles.ImageListItem}>
            <ImageListItemUI item={item} key={item.id} />
          </div>
        ))}
      </div>
      <div className={styles.listBottom}></div>
      {images.length > 0 ? (
        <Link
          className={styles.nextPageLink}
          href={{
            pathname: router.pathname,
            query: { ...router.query, afterId: images[images.length - 1].id },
          }}
        >
          Next Page
        </Link>
      ) : null}
    </>
  );
}
