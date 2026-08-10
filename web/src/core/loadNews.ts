import { NewsListItem } from "@/core/NewsListItem";

const fetchNewsAttempt = async (
  pageSize: number,
  afterId: string | undefined,
  t: string | undefined,
  sort: string | undefined,
  searchTerm?: string,
  model?: string
): Promise<NewsListItem[]> => {
  const queryParams = new URLSearchParams({
    page_size: pageSize.toString(),
  });
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
  const response = await fetch(`/api/list?${queryParams}`, {
    method: "GET",
  });
  if (!response.ok) {
    throw new Error(response.statusText);
  }
  return (await response.json()) as NewsListItem[];
};

export const loadNews = async (
  pageSize: number,
  afterId: string | undefined,
  t: string | undefined,
  sort: string | undefined,
  searchTerm?: string,
  model?: string
): Promise<NewsListItem[]> => {
  let failCount = 0;
  for (;;) {
    try {
      return await fetchNewsAttempt(
        pageSize,
        afterId,
        t,
        sort,
        searchTerm,
        model
      );
    } catch (error) {
      console.log(error);
      if (++failCount > 10) {
        return [];
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 3000));
  }
};
