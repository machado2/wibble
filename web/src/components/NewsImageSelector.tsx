import { Tabs } from "antd";
import { useRouter } from "next/router";

export const NewsImageSelector = () => {
  const router = useRouter();

  const activeKey = router.pathname == "/images" ? "2" : "1";

  const onChange = (key: string) => {
    if (key != activeKey) {
      switch (key) {
        case "1":
          router.push({
            pathname: "/",
            query: router.query.lang ? { lang: router.query.lang } : {},
          });
          break;
        case "2":
          router.push({
            pathname: "/images",
            query: router.query.lang ? { lang: router.query.lang } : {},
          });
          break;
      }
    }
  };

  const items = [
    {
      label: "News",
      key: "1",
    },
    {
      label: "Images",
      key: "2",
    },
  ];

  return (
    <>
      <Tabs
        activeKey={activeKey}
        centered
        items={items}
        onChange={(key) => onChange(key)}
        size="small"
      />
    </>
  );
};
