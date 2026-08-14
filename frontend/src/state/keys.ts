import type { ArticleQueryParams } from "../api/types";

export const queryKeys = {
  overview: ["overview"] as const,
  feeds: ["feeds"] as const,
  sources: ["sources"] as const,
  articles: (params: ArticleQueryParams) => ["articles", params] as const,
  searchResults: (params: ArticleQueryParams) => ["articles", params, { mode: "search" }] as const,
  article: (id: string) => ["article", id] as const,
  saved: (params: { offset?: number; limit?: number }) => ["saved", params] as const,
  settings: ["settings"] as const,
  categories: ["categories"] as const,
  tags: ["tags"] as const,
  suggestions: (q: string) => ["suggestions", q] as const,
};
