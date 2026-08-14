import type { Headline } from "../api/types";

function formatDate(iso: string): string {
  const date = new Date(iso);
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export default function ArticleListItem({ item, hideFeed }: { item: Headline; hideFeed?: boolean }) {
  return (
    <div className="flex items-start justify-between gap-3 py-2">
      <div className="min-w-0">
        {item.title && <p className={`truncate text-sm ${item.unread ? "font-semibold text-zinc-100" : "text-zinc-300"}`}>{item.title}</p>}
        {!hideFeed && item.feed_title && <p className="truncate text-xs text-zinc-500">{item.feed_title}</p>}
      </div>
      <div className="flex shrink-0 items-center gap-2 text-xs text-zinc-500">
        {item.marked && <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] font-medium text-amber-400">Saved</span>}
        <span className="whitespace-nowrap">{item.date ? formatDate(item.date) : ""}</span>
      </div>
    </div>
  );
}
