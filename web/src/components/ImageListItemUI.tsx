import { BoxedImage } from "./BoxedImage";
import styles from "./ImageList.module.css";
import { ImageListItem } from "@/pages/api/images";

export const ImageListItemUI = (props: { item: ImageListItem }) => {
  const { item } = props;

  return (
    <>
      <BoxedImage
        prompt={item.prompt}
        alt={item.prompt}
        className={styles.ImageListItemImg}
      />
    </>
  );
};
