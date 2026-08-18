import { Link } from "react-router";
import { encodeId } from "../api/client";

export default function FeedNameLink({
  feedId,
  title,
  className,
}: {
  feedId: string;
  title?: string | null;
  className?: string;
}) {
  if (!title) return null;
  return (
    <Link
      to={`/feeds?feed=${encodeId(feedId)}`}
      className={className}
      onClick={(e) => e.stopPropagation()}
      onAuxClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      {title}
    </Link>
  );
}
