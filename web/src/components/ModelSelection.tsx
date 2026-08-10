import { Select } from "antd";
import styles from "@/styles/newsList.module.css";

export type OptionType = {
  value: string;
  label: string;
};

type ModelSelectionProps = {
  onChange: (value: string) => void;
  value?: string;
};

export const ModelSelection = (props: ModelSelectionProps) => {

  const options: OptionType[] = ["", "gpt-3.5-turbo", "gpt-4", "gpt-4-0314"].map(
    (m) => ({ value: m, label: m })
  );

  return (
    <>
      <Select
        value={props.value}
        options={options}
        style={{ width: 120 }}
        onChange={props.onChange}
        size="small"
        className={styles.sortSelector}
      />
    </>
  );
};
