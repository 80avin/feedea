import { useState } from "react";
import { Link } from "react-router";
import type { Headline } from "../api/types";
import { encodeId } from "../api/client";
import { formatAge } from "../utils/format";
import FeedAvatar from "./FeedAvatar";

function TimelineItem({ item }: { item: Headline }) {
  const [thumbFailed, setThumbFailed] = useState(false);

  return (
    <Link to={`/feeds/${encodeId(item.id)}`} className="block px-4 py-2 hover:bg-app-surface/60">
      <div className="flex items-start gap-3">
        <FeedAvatar feedId={item.feed_id} title={item.feed_title} className="h-8 w-8" />
        <div className="min-w-0 flex-1">
          {item.title && (
            <p className={`truncate text-sm ${item.unread ? "font-semibold text-app-text" : "text-app-text-2"}`}>
              {item.title}
            </p>
          )}
          <p className="truncate text-xs text-app-text-faint">
            {item.date ? formatAge(item.date) : ""}
            {item.date && item.feed_title ? " · " : ""}
            {item.feed_title}
          </p>
        </div>
        {item.thumbnail_url && !thumbFailed && (
          <img
            src={`/api/thumbnail/${encodeId(item.id)}`}
            alt=""
            loading="lazy"
            onError={() => setThumbFailed(true)}
            className="h-12 w-16 shrink-0 rounded-md object-cover"
          />
        )}
      </div>
    </Link>
  );
}

export default function TimelinePanel({
  articles,
  bottomRef,
}: {
  articles: Headline[];
  bottomRef?: (node: Element | null) => void;
}) {
  return (
    <div className="flex h-full flex-col">
      <ul className="divide-y divide-app-border/60">
        {articles.map((item) => (
          <li key={item.id}>
            <TimelineItem item={item} />
          </li>
        ))}
        {articles.length === 0 && <li className="py-8 text-center text-sm text-app-text-faint">No articles.</li>}
      </ul>
      <div ref={bottomRef} className="h-px" />
    </div>
  );
}
