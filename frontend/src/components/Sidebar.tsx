import type { ComponentType } from "react";
import { useState } from "react";
import { Link, NavLink, useLocation } from "react-router";
import { Button, Separator } from "@heroui/react";
import {
  BookmarkIcon,
  Cog6ToothIcon,
  FolderIcon,
  PlusIcon,
  QuestionMarkCircleIcon,
  RssIcon,
  ServerStackIcon,
  Squares2X2Icon,
} from "@heroicons/react/24/outline";
import clsx from "clsx";
import { useSession } from "../auth/useSession";
import { useCategories, useSources } from "../state/hooks";
import type { CategoryNode } from "../api/types";
import AddCategoryDialog from "./AddCategoryDialog";
import AddSourceDialog from "./AddSourceDialog";
import OpmlImportButton from "./OpmlImportButton";

type Icon = ComponentType<{ className?: string }>;

interface NavItemProps {
  to: string;
  label: string;
  icon: Icon;
  end?: boolean;
}

function NavItem({ to, label, icon: Icon, end }: NavItemProps) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        clsx(
          "flex items-center gap-2.5 rounded-md px-3 py-2 text-sm font-medium transition-colors",
          isActive
            ? "bg-accent-soft text-accent-soft-foreground"
            : "text-app-text-muted hover:bg-app-hover/60 hover:text-app-text",
        )
      }
    >
      <Icon className="h-4 w-4 shrink-0" />
      {label}
    </NavLink>
  );
}

function SectionHeading({
  icon: Icon,
  label,
  actions,
}: {
  icon: Icon;
  label: string;
  actions?: React.ReactNode;
}) {
  return (
    <h3 className="flex items-center justify-between gap-2 px-4 pb-1 pt-3">
      <span className="flex min-w-0 items-center gap-2 text-xs font-semibold uppercase tracking-wider text-app-text-faint">
        <Icon className="h-3.5 w-3.5 shrink-0" />
        <span className="truncate">{label}</span>
      </span>
      {actions && <span className="flex shrink-0 items-center gap-0.5">{actions}</span>}
    </h3>
  );
}

function IconButton({
  label,
  icon: Icon,
  onClick,
}: {
  label: string;
  icon: Icon;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="flex items-center justify-center rounded-md p-1 text-app-text-muted transition-colors hover:bg-app-hover/60 hover:text-app-text"
    >
      <Icon className="h-3.5 w-3.5" />
    </button>
  );
}

function TreeLink({ node, depth }: { node: CategoryNode; depth: number }) {
  return (
    <li>
      <NavLink
        to={`/feeds?category=${encodeURIComponent(node.category_id)}`}
        className={({ isActive }) =>
          clsx(
            "flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
            depth > 0 && "ml-3",
            isActive ? "bg-accent-soft text-accent-soft-foreground" : "text-app-text-muted hover:bg-app-hover/60 hover:text-app-text",
          )
        }
      >
        <span className="truncate">{node.name}</span>
        {node.unread_count > 0 && (
          <span className="rounded bg-app-surface-2 px-1.5 py-0.5 text-[10px] font-semibold text-app-text-2">
            {node.unread_count}
          </span>
        )}
      </NavLink>
      {node.children.length > 0 && (
        <ul className="mt-0.5 flex flex-col gap-0.5">
          {node.children.map((child) => (
            <TreeLink key={child.category_id} node={child} depth={depth + 1} />
          ))}
        </ul>
      )}
    </li>
  );
}

function FeedLink({ id, title, unreadCount }: { id: string; title: string; unreadCount: number }) {
  return (
    <NavLink
      to={`/feeds?feed=${encodeURIComponent(id)}`}
      className={({ isActive }) =>
        clsx(
          "flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
          isActive ? "bg-accent-soft text-accent-soft-foreground" : "text-app-text-muted hover:bg-app-hover/60 hover:text-app-text",
        )
      }
    >
      <span className="truncate">{title}</span>
      {unreadCount > 0 && (
        <span className="rounded bg-app-surface-2 px-1.5 py-0.5 text-[10px] font-semibold text-app-text-2">
          {unreadCount}
        </span>
      )}
    </NavLink>
  );
}

export default function Sidebar() {
  const location = useLocation();
  const { pathname } = location;
  const { logout } = useSession();
  const { data: categoriesData } = useCategories();
  const { data: sourcesData } = useSources();
  const feedsActive = pathname.startsWith("/feeds");
  const [addSourceOpen, setAddSourceOpen] = useState(false);
  const [addCategoryOpen, setAddCategoryOpen] = useState(false);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="border-b border-app-border px-4 py-3 text-lg font-bold tracking-tight">rssea</div>

      <nav className="flex flex-col gap-1 p-2">
        <NavItem to="/" label="Overview" icon={Squares2X2Icon} end />
        <NavItem to="/feeds" label="Feeds" icon={RssIcon} />
        <NavItem to="/saved" label="Saved" icon={BookmarkIcon} />
        <NavItem to="/sources" label="Sources" icon={ServerStackIcon} />
      </nav>

      {feedsActive && (
        <div className="flex min-h-0 flex-1 flex-col">
          <section className="flex min-h-0 flex-col">
            <SectionHeading
              icon={FolderIcon}
              label="Categories"
              actions={
                <IconButton label="Add category" icon={PlusIcon} onClick={() => setAddCategoryOpen(true)} />
              }
            />
            <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
              <ul className="flex flex-col gap-0.5">
                <li>
                  <Link
                    to="/feeds"
                    className="flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-sm text-app-text-muted hover:bg-app-hover/60 hover:text-app-text"
                  >
                    <span>All</span>
                  </Link>
                </li>
                {categoriesData?.categories.map((node) => (
                  <TreeLink key={node.category_id} node={node} depth={0} />
                ))}
              </ul>
            </div>
          </section>
          <Separator />
          <section className="flex min-h-0 flex-1 flex-col">
            <SectionHeading
              icon={ServerStackIcon}
              label="Sources"
              actions={
                <>
                  <OpmlImportButton />
                  <IconButton label="Add source" icon={PlusIcon} onClick={() => setAddSourceOpen(true)} />
                </>
              }
            />
            <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
              {sourcesData?.groups.map((group) => (
                <div key={group.category_id} className="mb-2">
                  <p className="px-2 py-1 text-xs text-app-text-faint">{group.category_name}</p>
                  <ul className="flex flex-col gap-0.5">
                    {group.feeds.map((feed) => (
                      <FeedLink key={feed.id} id={feed.id} title={feed.title} unreadCount={feed.unread_count} />
                    ))}
                  </ul>
                </div>
              ))}
            </div>
          </section>
        </div>
      )}

      {!feedsActive && <div className="flex-1" />}

      <Separator />
      <nav className="flex flex-col gap-1 p-2">
        <NavItem to="/help" label="Help" icon={QuestionMarkCircleIcon} />
        <NavItem to="/settings" label="Settings" icon={Cog6ToothIcon} />
      </nav>

      <div className="border-t border-app-border p-2">
        <Button variant="ghost" size="sm" fullWidth onPress={logout}>
          Log out
        </Button>
      </div>

      <AddSourceDialog open={addSourceOpen} onClose={() => setAddSourceOpen(false)} />
      <AddCategoryDialog open={addCategoryOpen} onClose={() => setAddCategoryOpen(false)} />
    </div>
  );
}
