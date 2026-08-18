import { useState } from "react";
import { Link } from "react-router";
import type { Headline } from "../api/types";
import { encodeId } from "../api/client";
import { formatAge } from "../utils/format";
import FeedAvatar from "./FeedAvatar";
import FeedContextMenu, { useFeedSummary } from "./FeedContextMenu";
import { DeleteDialog, RenameDialog } from "./SourceMenu";
import { useContextMenu } from "../hooks/useContextMenu";
import { useArticlePath } from "../hooks/useArticlePath";

function TimelineItem({ item }: { item: Headline }) {
  const [thumbFailed, setThumbFailed] = useState(false);
  const [dialog, setDialog] = useState<"rename" | "delete" | null>(null);
  const { position, close, menuRef, triggerProps } = useContextMenu();
  const feed = useFeedSummary(item.feed_id, item.feed_title ?? "");
  const articlePathFn = useArticlePath();

  return (
    <>
      <Link
        to={articlePathFn(item.id)}
        className="block px-4 py-2 hover:bg-app-surface/60"
        {...triggerProps}
      >
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
      {position && (
        <FeedContextMenu
          feed={feed}
          position={position}
          menuRef={menuRef}
          onClose={close}
          onEdit={() => setDialog("rename")}
          onDelete={() => setDialog("delete")}
        />
      )}
      <RenameDialog feed={feed} open={dialog === "rename"} onClose={() => setDialog(null)} />
      <DeleteDialog feed={feed} open={dialog === "delete"} onClose={() => setDialog(null)} />
    </>
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
