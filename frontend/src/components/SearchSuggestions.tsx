import type { Headline } from "../api/types";
import FeedAvatar from "./FeedAvatar";

export default function SearchSuggestions({
  suggestions,
  onSelect,
}: {
  suggestions: Headline[];
  onSelect: (id: string) => void;
}) {
  return (
    <ul className="absolute left-0 right-0 z-20 mt-1 overflow-hidden rounded-md border border-zinc-800 bg-zinc-950 shadow-xl">
      {suggestions.map((s) => (
        <li key={s.id}>
          <button
            type="button"
            onClick={() => onSelect(s.id)}
            className="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-zinc-800/60"
          >
            <FeedAvatar feedId={s.feed_id} title={s.feed_title} className="h-6 w-6" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm text-zinc-100">{s.title}</p>
              {s.feed_title && <p className="truncate text-xs text-zinc-500">{s.feed_title}</p>}
            </div>
          </button>
        </li>
      ))}
    </ul>
  );
}
