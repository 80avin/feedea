import { useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { PlusIcon, RssIcon } from "@heroicons/react/24/outline";
import { useArticles } from "../state/hooks";
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
