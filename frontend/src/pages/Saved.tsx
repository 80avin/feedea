import { useState } from "react";
import { Link } from "react-router";
import { PencilSquareIcon, BookmarkIcon, XMarkIcon } from "@heroicons/react/24/outline";
import { Chip } from "@heroui/react";
import { useSaved, useUnsaveArticle } from "../state/hooks";
import { encodeId } from "../api/client";
import { formatAge } from "../utils/format";
import { ErrorState, LoadingState } from "../components/Feedback";
import SaveDialog from "../components/SaveDialog";

function monthLabel(month: string): string {
  const [year, m] = month.split("-").map(Number);
  if (!year || !m) {
    return month;
  }
  return new Date(year, m - 1, 1).toLocaleDateString(undefined, { year: "numeric", month: "long" });
}

export default function Saved() {
  const { data, isLoading, isError, error, refetch } = useSaved();
  const unsave = useUnsaveArticle();
  const [editId, setEditId] = useState<string | null>(null);

  return (
    <div className="flex h-full flex-col p-4">
      <div className="flex items-center gap-2">
        <BookmarkIcon className="h-5 w-5 text-app-text-muted" />
        <h2 className="text-lg font-semibold">Saved</h2>
      </div>

      {isLoading && <div className="mt-4"><LoadingState /></div>}
      {isError && <div className="mt-4"><ErrorState error={error} onRetry={() => refetch()} /></div>}
      {data && (
        <div className="mt-4 flex-1 space-y-6 overflow-y-auto">
          {data.months.length === 0 && <p className="text-sm text-app-text-faint">Nothing saved yet.</p>}
          {data.months.map((group) => (
            <section key={group.month}>
              <h3 className="text-xs font-semibold uppercase tracking-wider text-app-text-faint">{monthLabel(group.month)}</h3>
              <ul className="divide-y divide-app-border/60">
                {group.items.map((item) => (
                  <li key={item.id} className="py-2">
                    <div className="flex items-start gap-2">
                      <Link to={`/feeds/${encodeId(item.id)}`} className="group min-w-0 flex-1">
                        {item.title && (
                          <p className={`truncate text-sm ${item.unread ? "font-semibold text-app-text" : "text-app-text-2"}`}>
                            {item.title}
                          </p>
                        )}
                        <p className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-app-text-faint">
                          {item.date && <span>{formatAge(item.date)}</span>}
                          {item.date && item.feed_title && <span aria-hidden="true">·</span>}
                          {item.feed_title && <span className="truncate">{item.feed_title}</span>}
                        </p>
                        {item.note && (
                          <p className="mt-1 line-clamp-2 text-xs text-app-text-muted">{item.note}</p>
                        )}
                        {item.tags && item.tags.length > 0 && (
                          <div className="mt-1.5 flex flex-wrap items-center gap-1">
                            {item.tags.map((tag) => (
                              <Chip key={tag} size="sm" variant="soft">{tag}</Chip>
                            ))}
                          </div>
                        )}
                      </Link>
                      <div className="flex shrink-0 items-center gap-1">
                        <button
                          type="button"
                          aria-label="Edit note and tags"
                          onClick={() => setEditId(item.id)}
                          className="rounded-md p-1.5 text-app-text-muted hover:bg-app-hover hover:text-app-text"
                        >
                          <PencilSquareIcon className="h-4 w-4" />
                        </button>
                        <button
                          type="button"
                          aria-label="Unsave"
                          onClick={() => unsave.mutate({ id: item.id })}
                          className="rounded-md p-1.5 text-app-text-muted hover:bg-app-hover hover:text-red-500 dark:hover:text-red-400"
                        >
                          <XMarkIcon className="h-4 w-4" />
                        </button>
                      </div>
                    </div>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      )}

      {editId && <SaveDialog open={editId !== null} articleId={editId} onClose={() => setEditId(null)} />}
    </div>
  );
}
