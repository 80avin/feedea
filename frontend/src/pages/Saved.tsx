import { BookmarkIcon } from "@heroicons/react/24/outline";
import { useSaved } from "../state/hooks";
import ArticleListItem from "../components/ArticleListItem";
import { ErrorState, LoadingState } from "../components/Feedback";

function monthLabel(month: string): string {
  const [year, m] = month.split("-").map(Number);
  if (!year || !m) {
    return month;
  }
  return new Date(year, m - 1, 1).toLocaleDateString(undefined, { year: "numeric", month: "long" });
}

export default function Saved() {
  const { data, isLoading, isError, error, refetch } = useSaved();

  return (
    <div className="flex h-full flex-col p-4">
      <div className="flex items-center gap-2">
        <BookmarkIcon className="h-5 w-5 text-zinc-400" />
        <h2 className="text-lg font-semibold">Saved</h2>
      </div>

      {isLoading && <div className="mt-4"><LoadingState /></div>}
      {isError && <div className="mt-4"><ErrorState error={error} onRetry={() => refetch()} /></div>}
      {data && (
        <div className="mt-4 flex-1 space-y-6 overflow-y-auto">
          {data.months.length === 0 && <p className="text-sm text-zinc-600">Nothing saved yet.</p>}
          {data.months.map((group) => (
            <section key={group.month}>
              <h3 className="text-xs font-semibold uppercase tracking-wider text-zinc-500">{monthLabel(group.month)}</h3>
              <ul className="divide-y divide-zinc-800/60">
                {group.items.map((item) => (
                  <ArticleListItem key={item.id} item={item} hideFeed />
                ))}
              </ul>
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
