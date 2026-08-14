import type { ComponentType } from "react";
import { Link, NavLink, useLocation, useNavigate } from "react-router";
import { Button, Separator } from "@heroui/react";
import {
  BookmarkIcon,
  Cog6ToothIcon,
  FolderIcon,
  QuestionMarkCircleIcon,
  RssIcon,
  ServerStackIcon,
  Squares2X2Icon,
} from "@heroicons/react/24/outline";
import clsx from "clsx";
import { useSession } from "../auth/useSession";
import { useCategories, useSources } from "../state/hooks";
import type { CategoryNode } from "../api/types";

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
          isActive ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100",
        )
      }
    >
      <Icon className="h-4 w-4 shrink-0" />
      {label}
    </NavLink>
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
            isActive ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100",
          )
        }
      >
        <span className="truncate">{node.name}</span>
        {node.unread_count > 0 && (
          <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] font-semibold text-zinc-300">
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
          isActive ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100",
        )
      }
    >
      <span className="truncate">{title}</span>
      {unreadCount > 0 && (
        <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] font-semibold text-zinc-300">
          {unreadCount}
        </span>
      )}
    </NavLink>
  );
}

export default function Sidebar() {
  const location = useLocation();
  const { pathname } = location;
  const navigate = useNavigate();
  const { logout } = useSession();
  const { data: categoriesData } = useCategories();
  const { data: sourcesData } = useSources();
  const feedsActive = pathname.startsWith("/feeds");

  const openSources = () => {
    navigate("/sources", { state: { backgroundLocation: location } });
  };

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="border-b border-zinc-800 px-4 py-3 text-lg font-bold tracking-tight">rssea</div>

      <nav className="flex flex-col gap-1 p-2">
        <NavItem to="/" label="Overview" icon={Squares2X2Icon} end />
        <NavItem to="/feeds" label="Feeds" icon={RssIcon} />
        <NavItem to="/saved" label="Saved" icon={BookmarkIcon} />
        <button
          type="button"
          onClick={openSources}
          className="flex items-center gap-2.5 rounded-md px-3 py-2 text-sm font-medium text-zinc-400 transition-colors hover:bg-zinc-800/60 hover:text-zinc-100"
        >
          <ServerStackIcon className="h-4 w-4 shrink-0" />
          Sources
        </button>
      </nav>

      {feedsActive && (
        <div className="flex min-h-0 flex-1 flex-col">
          <section className="flex min-h-0 flex-1 flex-col">
            <h3 className="flex items-center gap-2 px-4 pb-1 pt-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">
              <FolderIcon className="h-3.5 w-3.5" />
              Categories
            </h3>
            <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
              <ul className="flex flex-col gap-0.5">
                <li>
                  <Link
                    to="/feeds"
                    className="flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-sm text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100"
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
            <h3 className="flex items-center gap-2 px-4 pb-1 pt-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">
              <ServerStackIcon className="h-3.5 w-3.5" />
              Sources
            </h3>
            <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
              {sourcesData?.groups.map((group) => (
                <div key={group.category_id} className="mb-2">
                  <p className="px-2 py-1 text-xs text-zinc-600">{group.category_name}</p>
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

      <div className="border-t border-zinc-800 p-2">
        <Button variant="ghost" size="sm" fullWidth onPress={logout}>
          Log out
        </Button>
      </div>
    </div>
  );
}
