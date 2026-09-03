import { Fragment, useState } from "react";
import { Link } from "react-router";
import { Badge, Button, Popover } from "@heroui/react";
import {
  ExclamationTriangleIcon,
  PlusIcon,
  ServerStackIcon,
} from "@heroicons/react/24/outline";
import { encodeId } from "../api/client";
import type { FeedSummary, SourceGroup } from "../api/types";
import { useCategories, useSources } from "../state/hooks";
import { categoryPathFromLeaf } from "../utils/categories";
import AddSourceDialog from "../components/AddSourceDialog";
import OpmlButtons from "../components/OpmlButtons";
import SourceMenu from "../components/SourceMenu";
import FeedAvatar from "../components/FeedAvatar";
import { ErrorState, LoadingState } from "../components/Feedback";

function CategoryHeading({ group }: { group: SourceGroup }) {
  const { data: categoriesData } = useCategories();
  const segments = categoryPathFromLeaf(
    categoriesData?.categories ?? [],
    group.category_id,
  );
  if (segments.length === 0) {
    return (
      <h3 className="px-2 pb-1 text-xs font-semibold uppercase tracking-wider text-app-text-faint">
        {group.category_name}
      </h3>
    );
  }
  return (
    <h3 className="flex flex-wrap items-center gap-x-1 px-2 pb-1 text-xs font-semibold uppercase tracking-wider text-app-text-faint">
      {segments.map((segment, index) => (
        <Fragment key={segment.id}>
          {index > 0 && <span aria-hidden>{" / "}</span>}
          <Link
            to={`/feeds?category=${encodeURIComponent(segment.id)}`}
            className="rounded px-0.5 hover:bg-app-hover/60 hover:text-app-text"
          >
            {segment.name}
          </Link>
        </Fragment>
      ))}
    </h3>
  );
}

function SourceRow({ feed }: { feed: FeedSummary }) {
  return (
    <li className="flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-app-hover/50">
      <FeedAvatar feedId={feed.id} title={feed.title} className="h-6 w-6" />
      <Link
        to={`/feeds?feed=${encodeId(feed.id)}`}
        className="min-w-0 flex-1 truncate text-sm text-app-text-2 hover:text-app-text"
        title={feed.title}
      >
        {feed.title}
      </Link>
      {feed.error_message && (
        <Popover>
          <Button variant="ghost">
            <Badge.Anchor>
              <ExclamationTriangleIcon
                className="h-4 w-4 shrink-0 text-amber-500 dark:text-amber-400"
                title={feed.error_message}
                aria-label="This source has errors"
              />
              <Badge size="sm">{feed.error_count}</Badge>
            </Badge.Anchor>
          </Button>
          <Popover.Content>
            <Popover.Dialog>
              <Popover.Heading>
                {feed.error_count > 1 && feed.error_count} Error
                {feed.error_count > 1 && "s"}
              </Popover.Heading>
              <p className="">{feed.error_message}</p>
            </Popover.Dialog>
          </Popover.Content>
        </Popover>
      )}
      {feed.unread_count > 0 && (
        <span className="shrink-0 rounded bg-app-surface-2 px-1.5 py-0.5 text-[10px] font-semibold text-app-text-2">
          {feed.unread_count}
        </span>
      )}
      <SourceMenu feed={feed} />
    </li>
  );
}

export default function Sources() {
  const { data, isLoading, isError, error, refetch } = useSources();
  const [addOpen, setAddOpen] = useState(false);
  const groups = data?.groups ?? [];
  const totalFeeds = groups.reduce((sum, group) => sum + group.feeds.length, 0);

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-wrap items-center justify-between gap-x-2 gap-y-2 px-4 pt-4">
        <div className="flex items-center gap-2">
          <ServerStackIcon className="h-5 w-5 text-app-text-muted" />
          <h2 className="text-lg font-semibold">Sources</h2>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <OpmlButtons />
          <Button size="sm" variant="primary" onPress={() => setAddOpen(true)}>
            <PlusIcon className="h-4 w-4" />
            Add source
          </Button>
        </div>
      </div>

      <div className="mt-3 min-h-0 flex-1 overflow-y-auto px-4 pb-4">
        {isLoading && <LoadingState />}
        {isError && <ErrorState error={error} onRetry={() => refetch()} />}
        {data && totalFeeds === 0 && (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <p className="text-sm text-app-text-muted">No sources yet.</p>
            <Button
              size="sm"
              variant="primary"
              onPress={() => setAddOpen(true)}
            >
              Add your first source
            </Button>
          </div>
        )}
        {data &&
          totalFeeds > 0 &&
          groups.map((group) => (
            <section key={group.category_id} className="mb-4">
              <CategoryHeading group={group} />
              <ul className="flex flex-col gap-0.5">
                {group.feeds.map((feed) => (
                  <SourceRow key={feed.id} feed={feed} />
                ))}
              </ul>
            </section>
          ))}
      </div>

      <AddSourceDialog open={addOpen} onClose={() => setAddOpen(false)} />
    </div>
  );
}
