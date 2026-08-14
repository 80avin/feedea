import { Link } from "react-router";
import { ArrowRightIcon, Squares2X2Icon } from "@heroicons/react/24/outline";
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
}: {
  linkTo: string | null;
  name: string;
  totalCount: number;
  unreadCount: number;
  items: Headline[];
}) {
  return (
    <section className="flex flex-col overflow-hidden rounded-lg border border-zinc-800">
      <header className="flex items-center justify-between gap-2 border-b border-zinc-800 px-3 py-2">
        <h3 className="text-sm font-semibold">{name}</h3>
        <span className="text-xs text-zinc-500">
          {unreadCount > 0 ? (
            <>
              <span className="font-semibold text-zinc-100">{unreadCount}</span> unread ·{" "}
            </>
          ) : null}
          {totalCount} total
        </span>
      </header>
      <ul className="flex-1 divide-y divide-zinc-800/60 px-3 py-1">
        {items.map((item) => (
          <ArticleListItem key={item.id} item={item} />
        ))}
        {items.length === 0 && <li className="py-4 text-sm text-zinc-600">No articles yet.</li>}
      </ul>
      {linkTo && (
        <Link
          to={linkTo}
          className="flex items-center gap-1 border-t border-zinc-800 px-3 py-1.5 text-xs font-medium text-zinc-400 hover:text-zinc-100"
        >
          More
          <ArrowRightIcon className="h-3.5 w-3.5" />
        </Link>
      )}
    </section>
  );
}

export default function Overview() {
  const { data, isLoading, isError, error, refetch } = useOverview();

  return (
    <div className="flex h-full flex-col p-4">
      <div className="flex items-center gap-2">
        <Squares2X2Icon className="h-5 w-5 text-zinc-400" />
        <h2 className="text-lg font-semibold">Overview</h2>
      </div>

      {isLoading && <div className="mt-4"><LoadingState /></div>}
      {isError && <div className="mt-4"><ErrorState error={error} onRetry={() => refetch()} /></div>}
      {data && (
        <div className="mt-4 grid flex-1 grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {data.cards.map((card) => (
            <Card
              key={card.category_id}
              linkTo={`/feeds?category=${encodeURIComponent(card.category_id)}`}
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
          />
        </div>
      )}
    </div>
  );
}
