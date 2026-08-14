import { useState } from "react";
import { encodeId } from "../api/client";

export default function FeedAvatar({
  feedId,
  title,
  className,
}: {
  feedId: string;
  title: string | null;
  className?: string;
}) {
  const [failed, setFailed] = useState(false);

  if (failed) {
    const letter = (title?.trim()?.[0] ?? "?").toUpperCase();
    return (
      <div className={`flex shrink-0 items-center justify-center rounded-full bg-zinc-800 font-semibold text-zinc-400 ${className ?? ""}`}>
        {letter}
      </div>
    );
  }

  return (
    <img
      src={`/api/favicon/${encodeId(feedId)}`}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
      className={`shrink-0 rounded ${className ?? ""}`}
    />
  );
}
