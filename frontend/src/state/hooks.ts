import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, apiGetText, encodeId } from "../api/client";
import type {
  ArticleDetail,
  ArticleQueryParams,
  CategoriesResponse,
  FeedSummary,
  Headline,
  OverviewResponse,
  SavedResponse,
  Settings,
  SourcesResponse,
  SuggestionsResponse,
  TagsResponse,
} from "../api/types";
import { queryKeys } from "./keys";

const PAGE_SIZE = 30;

function serializeParams(params: Record<string, string | undefined>): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== "") {
      query.set(key, value);
    }
  }
  const str = query.toString();
  return str ? `?${str}` : "";
}

function articlesPath(params: ArticleQueryParams, offset: number, limit: number): string {
  return `/api/articles${serializeParams({ ...params, offset: String(offset), limit: String(limit) })}`;
}

export function useOverview() {
  return useQuery({ queryKey: queryKeys.overview, queryFn: () => api.get<OverviewResponse>("/api/overview") });
}

export function useFeeds() {
  return useQuery({ queryKey: queryKeys.feeds, queryFn: () => api.get<FeedSummary[]>("/api/feeds") });
}

export function useSources() {
  return useQuery({ queryKey: queryKeys.sources, queryFn: () => api.get<SourcesResponse>("/api/sources") });
}

export function useCategories() {
  return useQuery({
    queryKey: queryKeys.categories,
    queryFn: () => api.get<CategoriesResponse>("/api/categories"),
  });
}

export function useTags() {
  return useQuery({ queryKey: queryKeys.tags, queryFn: () => api.get<TagsResponse>("/api/tags") });
}

export function useSuggestions(q: string): { suggestions: Headline[]; isFetching: boolean } {
  const trimmed = q.trim();
  const { data, isFetching } = useQuery({
    queryKey: queryKeys.suggestions(trimmed),
    queryFn: () => api.get<SuggestionsResponse>(`/api/search/suggestions?q=${encodeURIComponent(trimmed)}`),
    enabled: trimmed.length > 0,
  });
  return { suggestions: data?.suggestions ?? [], isFetching };
}

export function useSettings() {
  return useQuery({ queryKey: queryKeys.settings, queryFn: () => api.get<Settings>("/api/settings") });
}

interface ArticlesQueryResult {
  data: { pages: Headline[][] } | undefined;
  isLoading: boolean;
  isError: boolean;
  error: unknown;
  refetch: () => void;
  fetchNextPage: () => void;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
}

export function useArticles(params: ArticleQueryParams): ArticlesQueryResult {
  const hasSearch = !!(params.search && params.search.trim().length > 0);
  const infinite = useInfiniteQuery({
    queryKey: queryKeys.articles(params),
    queryFn: ({ pageParam }) => api.get<Headline[]>(articlesPath(params, pageParam as number, PAGE_SIZE)),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) => {
      if (lastPage.length < PAGE_SIZE) {
        return undefined;
      }
      return allPages.reduce((total, page) => total + page.length, 0);
    },
    enabled: !hasSearch,
  });
  const search = useQuery({
    queryKey: queryKeys.searchResults(params),
    queryFn: () => api.get<Headline[]>(articlesPath(params, 0, PAGE_SIZE)),
    enabled: hasSearch,
  });
  if (hasSearch) {
    return {
      data: search.data ? { pages: [search.data] } : undefined,
      isLoading: search.isLoading,
      isError: search.isError,
      error: search.error,
      refetch: search.refetch,
      fetchNextPage: () => {},
      hasNextPage: false,
      isFetchingNextPage: false,
    };
  }
  return {
    data: infinite.data,
    isLoading: infinite.isLoading,
    isError: infinite.isError,
    error: infinite.error,
    refetch: infinite.refetch,
    fetchNextPage: infinite.fetchNextPage,
    hasNextPage: infinite.hasNextPage,
    isFetchingNextPage: infinite.isFetchingNextPage,
  };
}

export function useSaved(params: { offset?: number; limit?: number } = {}) {
  const query = serializeParams({
    offset: params.offset !== undefined ? String(params.offset) : undefined,
    limit: params.limit !== undefined ? String(params.limit) : undefined,
  });
  return useQuery({ queryKey: queryKeys.saved(params), queryFn: () => api.get<SavedResponse>(`/api/saved${query}`) });
}

export function useArticle(id: string) {
  return useQuery({
    queryKey: queryKeys.article(id),
    queryFn: () => api.get<ArticleDetail>(`/api/articles/${encodeId(id)}`),
    enabled: id.length > 0,
  });
}

export function useSaveArticle() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, note, tags }: { id: string; note?: string; tags?: string[] }) =>
      api.post(`/api/articles/${encodeId(id)}/save`, { note, tags: tags ?? [] }),
    onSuccess: (_data, { id }) => {
      queryClient.invalidateQueries({ queryKey: ["articles"] });
      queryClient.invalidateQueries({ queryKey: ["saved"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
      queryClient.invalidateQueries({ queryKey: ["article", id] });
      queryClient.invalidateQueries({ queryKey: ["tags"] });
    },
  });
}

export function useUnsaveArticle() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id }: { id: string }) => api.delete(`/api/articles/${encodeId(id)}/save`),
    onSuccess: (_data, { id }) => {
      queryClient.invalidateQueries({ queryKey: ["articles"] });
      queryClient.invalidateQueries({ queryKey: ["saved"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
      queryClient.invalidateQueries({ queryKey: ["article", id] });
      queryClient.invalidateQueries({ queryKey: ["tags"] });
    },
  });
}

export function useUpdateNoteTags() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, note, tags }: { id: string; note?: string; tags?: string[] }) =>
      api.put(`/api/articles/${encodeId(id)}/save`, { note, tags: tags ?? [] }),
    onSuccess: (_data, { id }) => {
      queryClient.invalidateQueries({ queryKey: ["articles"] });
      queryClient.invalidateQueries({ queryKey: ["saved"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
      queryClient.invalidateQueries({ queryKey: ["article", id] });
      queryClient.invalidateQueries({ queryKey: ["tags"] });
    },
  });
}

export function useMarkRead() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, read }: { id: string; read: boolean }) =>
      api.post(`/api/articles/${encodeId(id)}/${read ? "read" : "unread"}`, read ? { read: true } : undefined),
    onSuccess: (_data, { id }) => {
      queryClient.invalidateQueries({ queryKey: ["articles"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
      queryClient.invalidateQueries({ queryKey: ["article", id] });
      queryClient.invalidateQueries({ queryKey: ["categories"] });
      queryClient.invalidateQueries({ queryKey: ["sources"] });
    },
  });
}

export function useMarkAllRead() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api.post("/api/read-all"),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["articles"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
      queryClient.invalidateQueries({ queryKey: ["categories"] });
      queryClient.invalidateQueries({ queryKey: ["sources"] });
    },
  });
}

export function useRefreshFeed() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id }: { id: string }) => api.post(`/api/sources/${encodeId(id)}/refresh`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["articles"] });
      queryClient.invalidateQueries({ queryKey: ["feeds"] });
      queryClient.invalidateQueries({ queryKey: ["sources"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
    },
  });
}

export function useAddSource() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ url, title, category_id }: { url: string; title?: string; category_id?: string }) =>
      api.post<FeedSummary>("/api/sources", { url, title, category_id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["feeds"] });
      queryClient.invalidateQueries({ queryKey: ["sources"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useDeleteFeed() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id }: { id: string }) => api.delete(`/api/sources/${encodeId(id)}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["feeds"] });
      queryClient.invalidateQueries({ queryKey: ["sources"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useAddCategory() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ name, parent_id }: { name: string; parent_id?: string }) =>
      api.post("/api/categories", { name, parent_id }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["categories"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
    },
  });
}

export function useDeleteCategory() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, remove_children }: { id: string; remove_children?: boolean }) =>
      api.delete(`/api/categories/${encodeId(id)}`, { remove_children: remove_children ?? false }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["categories"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
      queryClient.invalidateQueries({ queryKey: ["articles"] });
      queryClient.invalidateQueries({ queryKey: ["saved"] });
    },
  });
}

export function useRenameCategory() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) => api.patch(`/api/categories/${encodeId(id)}`, { name }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["categories"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
    },
  });
}

export function useMarkCategoryRead() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id }: { id: string }) => api.post(`/api/categories/${encodeId(id)}/read`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["categories"] });
      queryClient.invalidateQueries({ queryKey: ["articles"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
    },
  });
}

export function useChangePassword() {
  return useMutation({
    mutationFn: ({ current_password, new_password }: { current_password: string; new_password: string }) =>
      api.post("/api/settings/password", { current_password, new_password }),
  });
}

export function useUpdateSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (patch: { theme?: string; sync_interval_minutes?: number; keep_articles_days?: number | null }) =>
      api.patch("/api/settings", patch),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}

export function useImportOpml() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ opml }: { opml: string }) => api.post("/api/sources/import-opml", { opml }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["feeds"] });
      queryClient.invalidateQueries({ queryKey: ["sources"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
    },
  });
}

export function useExportOpml() {
  return useMutation({
    mutationFn: async () => apiGetText("/api/sources/export-opml"),
  });
}

export function useDiscover() {
  return useMutation({
    mutationFn: ({ url }: { url: string }) => api.post("/api/sources/discover", { url }),
  });
}
