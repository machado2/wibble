import { useRouter } from "next/router";

export const useUpdateUrlQuery = () => {
  const router = useRouter();

  const updateUrlQuery = (parameter: string, value?: string) => {
    const newQuery = router.query;
    const oldValue = newQuery[parameter];
    if (value === undefined || value === "") {
      if (oldValue === undefined) {
        return;
      }
      delete newQuery[parameter];
    } else {
      if (oldValue === value) {
        return;
      }
      newQuery[parameter] = value;
    }
    router.push(
      {
        pathname: router.pathname,
        query: newQuery,
      },
      undefined,
      { shallow: true }
    );
  };

  return {
    updateUrlQuery,
  };
};
