// NewsFetcher.ts

import { NewsListItem } from "@/core/NewsListItem";
import { dontWaitFor } from "@/core/dontWaitFor";

class NewsFetcher {
  private _hasMore: boolean = true;
  private _isLoading: boolean = false;

  constructor(
    private pageSize: number,
    private lastCreatedAt: string | undefined,
    private lastHotScore: number | undefined,
    private searchTerm: string | undefined,
    private news: NewsListItem[],
    private model: string | undefined,
    private newsChanged: (news: NewsListItem[]) => void,
    private hasMoreChanged: (hasMore: boolean) => void,
    private isLoadingChanged: (isLoading: boolean) => void
  ) {
    this.hasMore = news.length >= pageSize;
    if (Number.isNaN(this.lastHotScore)) {
      this.lastHotScore = 0;
    }
    if (news.length === 0) {
      dontWaitFor(this.loadNews());
    }
  }

  async loadNews(): Promise<void> {
    const newNews = await this.fetchNews();
    this.setNews(newNews);
  }

  get hasMore(): boolean {
    return this._hasMore;
  }

  set hasMore(value: boolean) {
    if (value !== this._hasMore) {
      this._hasMore = value;
      this.hasMoreChanged(value);
    }
  }

  get isLoading(): boolean {
    return this._isLoading;
  }

  set isLoading(value: boolean) {
    if (value !== this._isLoading) {
      this._isLoading = value;
      this.isLoadingChanged(value);
    }
  }

  private setNews(news: NewsListItem[]) {
    this.news = news;
    this.newsChanged(news);
  }

  private setIsLoading(isLoading: boolean) {
    if (isLoading !== this.isLoading) {
      this.isLoading = isLoading;
      this.isLoadingChanged(isLoading);
    }
  }

  private async fetchNewsAttempt(): Promise<NewsListItem[]> {
    const queryParams = new URLSearchParams({
      page_size: this.pageSize.toString(),
    });
    if (this.lastHotScore !== undefined) {
      queryParams.set("last_hot_score", this.lastHotScore.toString());
    }
    if (this.lastCreatedAt !== undefined) {
      queryParams.set("last_created_at", this.lastCreatedAt);
    }
    if (this.searchTerm && this.searchTerm.length > 0) {
      queryParams.set("search", this.searchTerm);
    }
    if (this.model && this.model.length > 0) {
      queryParams.set("model", this.model);
    }
    const response = await fetch(`/api/latest?${queryParams}`, {
      method: "GET",
    });
    if (!response.ok) {
      throw new Error(response.statusText);
    }
    const newNews = (await response.json()) as NewsListItem[];
    this.hasMore = newNews.length >= this.pageSize;
    return newNews;
  }

  private async fetchNews(): Promise<NewsListItem[]> {
    let failCount = 0;
    this.setIsLoading(true);
    try {
      for (;;) {
        try {
          return await this.fetchNewsAttempt();
        } catch (error) {
          console.log(error);
          if (++failCount > 10) {
            this.hasMore = false;
            return [];
          }
        }
        await new Promise((resolve) => setTimeout(resolve, 3000));
      }
    } finally {
      this.setIsLoading(false);
    }
  }

  async setSearchTerm(searchTerm: string | undefined): Promise<void> {
    if (searchTerm !== this.searchTerm) {
      this.searchTerm = searchTerm;
      this.lastHotScore = undefined;
      this.lastCreatedAt = undefined;
      const newNews = await this.fetchNews();
      this.setNews(newNews);
    }
  }

  async handleScroll() {
    if (this.hasMore && !this.isLoading) {
      const news = this.news;
      this.lastHotScore = news[news.length - 1]?.hotScore;
      this.lastCreatedAt = news[news.length - 1]?.created_at;
      const newNews = await this.fetchNews();
      this.setNews([...news, ...newNews]);
    }
  }
}

export default NewsFetcher;
