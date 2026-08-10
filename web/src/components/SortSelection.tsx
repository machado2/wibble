import { Select } from "antd";
import { useRouter } from "next/router";
import { useEffect, useState } from "react";
import styles from "@/styles/newsList.module.css";

export type OptionType = {
  value: string;
  label: string;
};

type SortSelectionProps = {
  options: OptionType[];
};

export const SortSelection = (props: SortSelectionProps) => {
  const router = useRouter();
  const { options } = props;
  const [sort, setSort] = useState<string | undefined>(undefined);

  useEffect(() => {
    const currentSort = router.query.sort as string | undefined;
    setSort(currentSort);
  }, [router.query.sort]);

  const onChange = (value: string) => {
    const query = { ...router.query };
    delete query.afterId;
    if (!value || value === "recent") delete query.t;
    if (!value) {
      delete query.sort;
      router.push({
        pathname: router.pathname,
        query,
      });
    } else if (sort !== value) {
      router.push({
        pathname: router.pathname,
        query: { ...query, sort: value },
      });
    }
  };

  return (
    <>
      <Select
        value={sort || options[0].value}
        options={options}
        style={{ width: 120 }}
        onChange={onChange}
        size="small"
        className={styles.sortSelector}
      />
    </>
  );
};
