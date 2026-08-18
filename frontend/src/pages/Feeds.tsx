import { useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { PlusIcon, RssIcon, XMarkIcon } from "@heroicons/react/24/outline";
import { useArticles, useCategories, useSources } from "../state/hooks";
import type { CategoryNode } from "../api/types";
import { useInfiniteScroll } from "../hooks/useInfiniteScroll";
import SearchBar from "../components/SearchBar";
import TimelinePanel from "../components/TimelinePanel";
import AddCategoryDialog from "../components/AddCategoryDialog";
import { ErrorState, LoadingState } from "../components/Feedback";

export default function Feeds() {
  const [searchParams] = useSearchParams();
  const q = searchParams.get("q") ?? "";
  const params = {
    feed: searchParams.get("feed") ?? undefined,
    category: searchParams.get("category") ?? undefined,
    saved: searchParams.get("saved") ?? undefined,
    unread: searchParams.get("unread") ?? undefined,
    tag: searchParams.get("tag") ?? undefined,
    search: q.trim() ? q : undefined,
  };
  const { data, isLoading, isError, error, refetch, fetchNextPage, hasNextPage, isFetchingNextPage } =
    useArticles(params);
  const articles = data?.pages.flatMap((page) => page) ?? [];
  const scrollRef = useRef<HTMLDivElement>(null);
  const [addCategoryOpen, setAddCategoryOpen] = useState(false);
  const sentinel = useInfiniteScroll(
    () => {
      if (hasNextPage && !isFetchingNextPage) {
        fetchNextPage();
      }
    },
    { rootRef: scrollRef },
  );
  const { chips, clearAll } = useFilterChips();

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between gap-2 px-4 pt-4">
        <div className="flex items-center gap-2">
          <RssIcon className="h-5 w-5 text-app-text-muted" />
          <h2 className="text-lg font-semibold">Feeds</h2>
        </div>
        <button
          type="button"
          onClick={() => setAddCategoryOpen(true)}
          className="flex items-center gap-1 rounded-md px-2 py-1 text-sm font-medium text-app-text-muted transition-colors hover:bg-app-hover/60 hover:text-app-text"
        >
          <PlusIcon className="h-4 w-4" />
          New category
        </button>
      </div>

      {chips.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5 px-4 pt-3">
          {chips.map((chip) => (
            <button
              key={chip.key}
              type="button"
              onClick={chip.remove}
              title="Remove filter"
              className="group inline-flex items-center gap-1 rounded-full border border-accent/40 bg-accent-soft px-2.5 py-0.5 text-xs font-medium text-accent-soft-foreground hover:bg-accent-soft-hover"
            >
              {chip.label}
              <XMarkIcon className="h-3 w-3" />
            </button>
          ))}
          {chips.length > 1 && (
            <button
              type="button"
              onClick={clearAll}
              className="text-xs font-medium text-app-text-muted hover:text-app-text"
            >
              Clear all
            </button>
          )}
        </div>
      )}

      <div className="px-4 pt-3">
        <SearchBar />
      </div>

      <div ref={scrollRef} className="mt-2 flex-1 overflow-y-auto">
        {isLoading && (
          <div className="p-4">
            <LoadingState />
          </div>
        )}
        {isError && (
          <div className="p-4">
            <ErrorState error={error} onRetry={() => refetch()} />
          </div>
        )}
        {data && <TimelinePanel articles={articles} bottomRef={sentinel} />}
      </div>

      <AddCategoryDialog open={addCategoryOpen} onClose={() => setAddCategoryOpen(false)} />
    </div>
  );
}

function useFilterChips() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { data: sourcesData } = useSources();
  const { data: categoriesData } = useCategories();

  const feedId = searchParams.get("feed");
  const categoryId = searchParams.get("category");
  const saved = searchParams.get("saved");
  const unread = searchParams.get("unread");
  const tag = searchParams.get("tag");
  const q = searchParams.get("q") ?? "";

  const feedName = useMemo(() => {
    const all = sourcesData?.groups.flatMap((g) => g.feeds) ?? [];
    return all.find((f) => f.id === feedId)?.title ?? feedId ?? "";
  }, [sourcesData, feedId]);

  const categoryName = useMemo(() => {
    const walk = (nodes: CategoryNode[]): string => {
      for (const node of nodes) {
        if (node.category_id === categoryId) return node.name;
        const found = walk(node.children);
        if (found) return found;
      }
      return "";
    };
    return categoriesData ? walk(categoriesData.categories) : "";
  }, [categoriesData, categoryId]);

  const remove = (key: string) => {
    const next = new URLSearchParams(searchParams);
    next.delete(key);
    setSearchParams(next);
  };
  const clearAll = () => {
    const next = new URLSearchParams(searchParams);
    for (const key of ["feed", "category", "saved", "unread", "tag", "q"]) {
      next.delete(key);
    }
    setSearchParams(next);
  };

  const chips: { key: string; label: string; remove: () => void }[] = [];
  if (feedId) chips.push({ key: "feed", label: feedName, remove: () => remove("feed") });
  if (categoryId) chips.push({ key: "category", label: categoryName || "Category", remove: () => remove("category") });
  if (q.trim()) chips.push({ key: "q", label: `“${q.trim()}”`, remove: () => remove("q") });
  if (saved) chips.push({ key: "saved", label: "Saved", remove: () => remove("saved") });
  if (unread) chips.push({ key: "unread", label: "Unread", remove: () => remove("unread") });
  if (tag) chips.push({ key: "tag", label: `#${tag}`, remove: () => remove("tag") });

  return { chips, clearAll };
}
