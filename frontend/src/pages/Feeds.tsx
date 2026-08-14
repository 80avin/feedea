import { useSearchParams } from "react-router";
import { Button } from "@heroui/react";
import { RssIcon } from "@heroicons/react/24/outline";
import { useArticles } from "../state/hooks";
import TimelinePanel from "../components/TimelinePanel";
import { ErrorState, LoadingState } from "../components/Feedback";

export default function Feeds() {
  const [searchParams] = useSearchParams();
  const params = {
    feed: searchParams.get("feed") ?? undefined,
    category: searchParams.get("category") ?? undefined,
    saved: searchParams.get("saved") ?? undefined,
    unread: searchParams.get("unread") ?? undefined,
    tag: searchParams.get("tag") ?? undefined,
    search: searchParams.get("q") ?? undefined,
  };
  const { data, isLoading, isError, error, refetch, fetchNextPage, hasNextPage, isFetchingNextPage } = useArticles(params);
  const articles = data?.pages.flatMap((page) => page) ?? [];

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 px-4 pt-4">
        <RssIcon className="h-5 w-5 text-zinc-400" />
        <h2 className="text-lg font-semibold">Feeds</h2>
      </div>

      <div className="mt-2 flex-1 overflow-y-auto">
        {isLoading && <div className="p-4"><LoadingState /></div>}
        {isError && <div className="p-4"><ErrorState error={error} onRetry={() => refetch()} /></div>}
        {data && (
          <>
            <TimelinePanel articles={articles} />
            {hasNextPage && (
              <div className="p-4">
                <Button
                  variant="primary"
                  size="sm"
                  fullWidth
                  isDisabled={isFetchingNextPage}
                  onPress={() => fetchNextPage()}
                >
                  {isFetchingNextPage ? "Loading…" : "Load more"}
                </Button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
