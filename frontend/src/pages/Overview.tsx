import { useMemo, useState } from "react";
import { Link } from "react-router";
import { ArrowRightIcon, Squares2X2Icon } from "@heroicons/react/24/outline";
import { encodeId } from "../api/client";
import { useOfflineWarmer } from "../pwa/useOfflineWarmer";
import { useOverview } from "../state/hooks";
import type { Headline } from "../api/types";
import ArticleListItem from "../components/ArticleListItem";
import { ErrorState, LoadingState } from "../components/Feedback";

type SortMode = "alpha" | "recent";
const SORT_KEY = "feedea:overview-sort";
const HIDE_KEY = "feedea:overview-hide-read";
function loadSort(): SortMode {
  return window.localStorage.getItem(SORT_KEY) === "recent" ? "recent" : "alpha";
}
function loadHide(): boolean {
  return window.localStorage.getItem(HIDE_KEY) === "1";
}
function cardRecency(c: { items: { date: string }[] }): number {
  return c.items.reduce((m, i) => Math.max(m, Date.parse(i.date) || 0), 0);
}

function Card({
  linkTo,
  name,
  totalCount,
  unreadCount,
  items,
  moreLabel = "More",
  showStats = false,
}: {
  linkTo: string | null;
  name: string;
  totalCount: number;
  unreadCount: number;
  items: Headline[];
  moreLabel?: string;
  showStats?: boolean;
}) {
  return (
    <section className="flex flex-col overflow-hidden rounded-lg border border-app-border">
      <header className="flex items-center justify-between gap-2 border-b border-app-border px-3 py-2">
        <h3 className="text-sm font-semibold">{name}</h3>
        <span className="text-xs text-app-text-faint">
          {unreadCount > 0 ? (
            <>
              <span className="font-semibold text-app-text">{unreadCount}</span> unread ·{" "}
            </>
          ) : null}
          {totalCount} total
        </span>
      </header>
      {showStats ? (
        <div className="flex flex-1 items-center justify-around px-3 py-6">
          <div className="text-center">
            <p className="text-2xl font-bold text-app-text">{unreadCount}</p>
            <p className="text-xs text-app-text-faint">unread</p>
          </div>
          <div className="text-center">
            <p className="text-2xl font-bold text-app-text">{totalCount}</p>
            <p className="text-xs text-app-text-faint">articles</p>
          </div>
        </div>
      ) : (
        <ul className="flex-1 divide-y divide-app-border/60 px-3 py-1">
          {items.map((item) => (
            <li key={item.id}>
              <ArticleListItem item={item} />
            </li>
          ))}
          {items.length === 0 && totalCount === 0 && <li className="py-4 text-sm text-app-text-faint">No articles yet.</li>}
          {items.length === 0 && totalCount > 0 && <li className="py-4 text-sm text-app-text-faint">All caught up.</li>}
        </ul>
      )}
      {linkTo && (
        <Link
          to={linkTo}
          className="flex items-center gap-1 border-t border-app-border px-3 py-1.5 text-xs font-medium text-app-text-muted hover:text-app-text"
        >
          {moreLabel}
          <ArrowRightIcon className="h-3.5 w-3.5" />
        </Link>
      )}
    </section>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
      <p className="text-sm text-app-text-muted">No feeds yet — add sources</p>
      <Link
        to="/sources"
        className="rounded-md bg-app-surface-2 px-3 py-1.5 text-sm font-medium text-app-text hover:bg-app-selected"
      >
        Add sources
      </Link>
    </div>
  );
}

export default function Overview() {
  const [sort, setSort] = useState<SortMode>(loadSort);
  const [hideRead, setHideRead] = useState(loadHide);
  const { data, isLoading, isError, error, refetch } = useOverview(hideRead);
  const cards = useMemo(() => {
    const list = data?.cards ?? [];
    if (sort === "recent") {
      return [...list].sort((a, b) => cardRecency(b) - cardRecency(a) || a.name.localeCompare(b.name));
    }
    return list;
  }, [data, sort]);

  const warmIds = useMemo(() => cards.flatMap((c) => c.items.map((i) => i.id)), [cards]);
  useOfflineWarmer(warmIds);

  const changeSort = (m: SortMode) => { setSort(m); window.localStorage.setItem(SORT_KEY, m); };
  const toggleHide = () => { setHideRead((v) => { window.localStorage.setItem(HIDE_KEY, v ? "0" : "1"); return !v; }); };

  return (
    <div className="flex h-full flex-col p-4">
      <div className="flex items-center gap-2">
        <Squares2X2Icon className="h-5 w-5 text-app-text-muted" />
        <h2 className="text-lg font-semibold">Overview</h2>
      </div>
      <div className="mt-3 flex items-center gap-2 text-xs">
        <div className="flex overflow-hidden rounded-md border border-app-border" role="tablist" aria-label="Sort categories">
          {(["alpha", "recent"] as const).map((m) => (
            <button key={m} type="button" role="tab" aria-selected={sort === m}
              onClick={() => changeSort(m)}
              className={sort === m ? "bg-app-surface-2 px-2.5 py-1 font-semibold text-app-text" : "px-2.5 py-1 text-app-text-muted hover:text-app-text"}>
              {m === "alpha" ? "A–Z" : "Recent"}
            </button>
          ))}
        </div>
        <label className="ml-auto flex cursor-pointer items-center gap-1.5 text-app-text-muted">
          <input type="checkbox" checked={hideRead} onChange={toggleHide} className="accent-[var(--accent)]" />
          Hide read
        </label>
      </div>

      {isLoading && <div className="mt-4"><LoadingState /></div>}
      {isError && <div className="mt-4"><ErrorState error={error} onRetry={() => refetch()} /></div>}
      {data &&
        (data.all.total_count === 0 ? (
          <EmptyState />
        ) : (
          <div className="mt-4 grid flex-1 grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {cards.map((card) => (
              <Card
                key={card.category_id}
                linkTo={`/feeds?category=${encodeId(card.category_id)}`}
                name={card.name}
                totalCount={card.total_count}
                unreadCount={card.unread_count}
                items={card.items}
              />
            ))}
            <Card
              linkTo="/feeds"
              name="All articles"
              totalCount={data.all.total_count}
              unreadCount={data.all.unread_count}
              items={[]}
              moreLabel="View all"
              showStats
            />
          </div>
        ))}
    </div>
  );
}
