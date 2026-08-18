import { useState } from "react";
import { useNavigate } from "react-router";
import { encodeId } from "../api/client";
import type { Headline } from "../api/types";
import ArticleContextMenu from "./ArticleContextMenu";
import { useContextMenu } from "../hooks/useContextMenu";
import { useArticlePath } from "../hooks/useArticlePath";
import FeedNameLink from "./FeedNameLink";

function formatDate(iso: string): string {
  const date = new Date(iso);
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export default function ArticleListItem({ item, hideFeed }: { item: Headline; hideFeed?: boolean }) {
  const [thumbFailed, setThumbFailed] = useState(false);
  const { position, close, menuRef, triggerProps } = useContextMenu();
  const articlePathFn = useArticlePath();
  const navigate = useNavigate();

  const openArticle = () => navigate(articlePathFn(item.id));
  const openArticleNewTab = (e: React.MouseEvent) => {
    if (e.button === 1) {
      e.preventDefault();
      window.open(articlePathFn(item.id), "_blank", "noopener");
    }
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      openArticle();
    }
  };

  return (
    <>
      <div
        role="link"
        tabIndex={0}
        className="block cursor-pointer hover:bg-app-surface/60"
        {...triggerProps}
        onClick={(e) => {
          if (triggerProps.onClick) triggerProps.onClick(e);
          if (!e.isPropagationStopped()) openArticle();
        }}
        onAuxClick={openArticleNewTab}
        onKeyDown={onKeyDown}
      >
        <div className="flex items-start justify-between gap-3 py-2">
          <div className="min-w-0 flex-1">
            {item.title && <p className={`truncate text-sm ${item.unread ? "font-semibold text-app-text" : "text-app-text-2"}`}>{item.title}</p>}
            {!hideFeed && item.feed_title && (
              <FeedNameLink
                feedId={item.feed_id}
                title={item.feed_title}
                className="truncate text-xs text-app-text-faint hover:text-accent hover:underline"
              />
            )}
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
      </div>
      {position && (
        <ArticleContextMenu article={item} position={position} menuRef={menuRef} onClose={close} />
      )}
    </>
  );
}
