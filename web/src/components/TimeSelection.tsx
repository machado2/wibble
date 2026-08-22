import { Select } from "antd";
import { useRouter } from "next/router";
import { useEffect, useState } from "react";
import { useGlobalLanguage } from "./GlobalLanguage";

export const TimeSelection = () => {
  const router = useRouter();
  const { copy } = useGlobalLanguage();
  const [period, setPeriod] = useState<string | undefined>(undefined);
  const options = [
    { value: "week", label: copy.periodWeek },
    { value: "month", label: copy.periodMonth },
    { value: "all", label: copy.periodAll },
  ];

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
