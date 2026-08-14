import { useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  ArrowPathIcon,
  ArrowTopRightOnSquareIcon,
  CheckCircleIcon,
  PencilIcon,
  TrashIcon,
} from "@heroicons/react/24/outline";
import type { FeedSummary } from "../api/types";
import { useFeedRead, useRefreshFeed, useSources } from "../state/hooks";
import type { ContextMenuPosition } from "../hooks/useContextMenu";
import { DeleteDialog, RenameDialog } from "./SourceMenu";

function useFeedSummary(feedId: string, fallbackTitle: string): FeedSummary {
  const { data } = useSources();
  return useMemo(() => {
    const all = data?.groups.flatMap((group) => group.feeds) ?? [];
    return (
      all.find((feed) => feed.id === feedId) ?? {
        id: feedId,
        title: fallbackTitle || "Feed",
        website: null,
        feed_url: null,
        icon_url: null,
        category_id: "",
        unread_count: 0,
        error_count: 0,
        error_message: null,
      }
    );
  }, [data, feedId, fallbackTitle]);
}

export default function FeedContextMenu({
  feedId,
  feedTitle,
  position,
  menuRef,
  onClose,
}: {
  feedId: string;
  feedTitle: string | null;
  position: ContextMenuPosition;
  menuRef: React.RefObject<HTMLDivElement | null>;
  onClose: () => void;
}) {
  const feed = useFeedSummary(feedId, feedTitle ?? "");
  const refresh = useRefreshFeed();
  const markRead = useFeedRead();
  const [editOpen, setEditOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const website = feed.website ?? feed.feed_url;
  const itemRef = useRef<HTMLDivElement>(null);
  const [clamped, setClamped] = useState(position);

  useLayoutEffect(() => {
    const el = itemRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const maxX = Math.max(4, window.innerWidth - rect.width - 4);
    const maxY = Math.max(4, window.innerHeight - rect.height - 4);
    const x = Math.min(Math.max(position.x, 4), maxX);
    const y = Math.min(Math.max(position.y, 4), maxY);
    setClamped({ x, y });
  }, [position]);

  const openSite = () => {
    if (website) window.open(website, "_blank", "noopener");
    onClose();
  };

  const itemClass =
    "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm text-app-text-2 hover:bg-app-hover hover:text-app-text disabled:cursor-not-allowed disabled:opacity-50";

  return (
    <>
      <div
        ref={(node) => {
          menuRef.current = node;
          itemRef.current = node;
        }}
        data-context-menu
        role="menu"
        className="fixed z-50 min-w-44 rounded-md border border-app-border-strong bg-app-bg py-1 shadow-xl"
        style={{ left: clamped.x, top: clamped.y }}
      >
        <button type="button" role="menuitem" className={itemClass} disabled={!website} onClick={openSite}>
          <ArrowTopRightOnSquareIcon className="h-4 w-4 shrink-0" />
          Open
        </button>
        <button
          type="button"
          role="menuitem"
          className={itemClass}
          onClick={() => {
            setEditOpen(true);
            onClose();
          }}
        >
          <PencilIcon className="h-4 w-4 shrink-0" />
          Edit
        </button>
        <button
          type="button"
          role="menuitem"
          className={itemClass}
          onClick={() => {
            refresh.mutate({ id: feed.id });
            onClose();
          }}
        >
          <ArrowPathIcon className="h-4 w-4 shrink-0" />
          Refresh
        </button>
        <button
          type="button"
          role="menuitem"
          className={itemClass}
          onClick={() => {
            markRead.mutate({ id: feed.id });
            onClose();
          }}
        >
          <CheckCircleIcon className="h-4 w-4 shrink-0" />
          Mark all read
        </button>
        <button
          type="button"
          role="menuitem"
          className={`${itemClass} text-red-600 hover:text-red-600 dark:text-red-400 dark:hover:text-red-400`}
          onClick={() => {
            setDeleteOpen(true);
            onClose();
          }}
        >
          <TrashIcon className="h-4 w-4 shrink-0" />
          Delete
        </button>
      </div>
      <RenameDialog feed={feed} open={editOpen} onClose={() => setEditOpen(false)} />
      <DeleteDialog feed={feed} open={deleteOpen} onClose={() => setDeleteOpen(false)} />
    </>
  );
}
