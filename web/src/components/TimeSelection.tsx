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
    if (!value) {
      let q = router.query;
      delete q.t;
      router.push({
        pathname: router.pathname,
        query: q,
      });
    } else if (period !== value) {
      router.push({
        pathname: router.pathname,
        query: { ...router.query, t: value },
      });
    }
  };

  return (
    <>
      <Select
        value={period?.length ?? 0 > 0 ? period : options[2].value}
        options={options}
        style={{ width: 120 }}
        onChange={onChange}
        size="small"
      />
    </>
  );
};
