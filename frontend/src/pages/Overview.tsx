import { Link } from "react-router";
import { ArrowRightIcon, Squares2X2Icon } from "@heroicons/react/24/outline";
import { encodeId } from "../api/client";
import { useOverview } from "../state/hooks";
import type { Headline } from "../api/types";
import ArticleListItem from "../components/ArticleListItem";
import { ErrorState, LoadingState } from "../components/Feedback";

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
          {items.length === 0 && <li className="py-4 text-sm text-app-text-faint">No articles yet.</li>}
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
  const { data, isLoading, isError, error, refetch } = useOverview();

  return (
    <div className="flex h-full flex-col p-4">
      <div className="flex items-center gap-2">
        <Squares2X2Icon className="h-5 w-5 text-app-text-muted" />
        <h2 className="text-lg font-semibold">Overview</h2>
      </div>

      {isLoading && <div className="mt-4"><LoadingState /></div>}
      {isError && <div className="mt-4"><ErrorState error={error} onRetry={() => refetch()} /></div>}
      {data &&
        (data.all.total_count === 0 ? (
          <EmptyState />
        ) : (
          <div className="mt-4 grid flex-1 grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {data.cards.map((card) => (
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
