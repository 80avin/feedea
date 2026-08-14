import { useState } from "react";
import { encodeId } from "../api/client";
import type { Headline } from "../api/types";

function formatDate(iso: string): string {
  const date = new Date(iso);
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export default function ArticleListItem({ item, hideFeed }: { item: Headline; hideFeed?: boolean }) {
  const [thumbFailed, setThumbFailed] = useState(false);
  return (
    <div className="flex items-start justify-between gap-3 py-2">
      <div className="min-w-0 flex-1">
        {item.title && <p className={`truncate text-sm ${item.unread ? "font-semibold text-app-text" : "text-app-text-2"}`}>{item.title}</p>}
        {!hideFeed && item.feed_title && <p className="truncate text-xs text-app-text-faint">{item.feed_title}</p>}
      </div>
      {item.thumbnail_url && !thumbFailed && (
        <img
          src={`/api/thumbnail/${encodeId(item.id)}`}
          alt=""
          loading="lazy"
          onError={() => setThumbFailed(true)}
          className="h-10 w-14 shrink-0 rounded-md object-cover"
        />
      )}
      <div className="flex shrink-0 items-center gap-2 text-xs text-app-text-faint">
        {item.marked && <span className="rounded bg-app-surface-2 px-1.5 py-0.5 text-[10px] font-medium text-amber-600 dark:text-amber-400">Saved</span>}
        <span className="whitespace-nowrap">{item.date ? formatDate(item.date) : ""}</span>
      </div>
    </div>
  );
}
