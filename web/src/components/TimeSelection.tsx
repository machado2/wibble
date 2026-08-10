import { Select } from "antd";
import { useRouter } from "next/router";
import { useEffect, useState } from "react";

const options = [
  {
    value: "week",
    label: "Week",
  },
  {
    value: "month",
    label: "Month",
  },
  {
    value: "all",
    label: "All",
  },
];

export const TimeSelection = () => {
  const router = useRouter();
  const [period, setPeriod] = useState<string | undefined>(undefined);

  useEffect(() => {
    const currentPeriod = router.query.t as string | undefined;
    setPeriod(currentPeriod);
  }, [router.query.t]);

  if (!router.query.sort || router.query.sort == 'recent') {
    return null;
  }

  const onChange = (value: string) => {
    const query = { ...router.query };
    delete query.afterId;
    if (!value) {
      delete query.t;
      router.push({
        pathname: router.pathname,
        query,
      });
    } else if (period !== value) {
      router.push({
        pathname: router.pathname,
        query: { ...query, t: value },
      });
    }
  };

  return (
    <>
      <Select
        value={period || options[2].value}
        options={options}
        style={{ width: 120 }}
        onChange={onChange}
        size="small"
      />
    </>
  );
};
