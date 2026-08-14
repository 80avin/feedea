import { avatarColorFor } from "../utils/avatarColor";

function initials(title: string | null): string {
  const trimmed = title?.trim();
  if (!trimmed) {
    return "?";
  }
  const words = trimmed.split(/\s+/);
  if (words.length === 1) {
    return words[0].slice(0, 2).toUpperCase();
  }
  return (words[0][0] + words[1][0]).toUpperCase();
}

export default function FeedAvatar({
  feedId,
  title,
  className,
}: {
  feedId: string;
  title: string | null;
  className?: string;
}) {
  return (
    <div
      className={`flex shrink-0 items-center justify-center rounded-full text-white ${className ?? ""}`}
      style={{ backgroundColor: avatarColorFor(feedId) }}
      aria-hidden="true"
    >
      <span className="text-[11px] font-semibold leading-none">{initials(title)}</span>
    </div>
  );
}
