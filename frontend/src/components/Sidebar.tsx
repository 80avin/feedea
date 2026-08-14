import type { ComponentType } from "react";
import { NavLink, useLocation } from "react-router";
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

export default function Sidebar() {
  const { pathname } = useLocation();
  const { logout } = useSession();
  const feedsActive = pathname.startsWith("/feeds");

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="border-b border-zinc-800 px-4 py-3 text-lg font-bold tracking-tight">rssea</div>

      <nav className="flex flex-col gap-1 p-2">
        <NavItem to="/" label="Overview" icon={Squares2X2Icon} end />
        <NavItem to="/feeds" label="Feeds" icon={RssIcon} />
        <NavItem to="/saved" label="Saved" icon={BookmarkIcon} />
      </nav>

      {feedsActive && (
        <div className="flex min-h-0 flex-1 flex-col">
          <section className="flex min-h-0 flex-1 flex-col">
            <h3 className="flex items-center gap-2 px-4 pb-1 pt-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">
              <FolderIcon className="h-3.5 w-3.5" />
              Categories
            </h3>
            <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
              <p className="px-3 py-2 text-sm text-zinc-600">Categories coming in Phase 5.</p>
            </div>
          </section>
          <Separator />
          <section className="flex min-h-0 flex-1 flex-col">
            <h3 className="flex items-center gap-2 px-4 pb-1 pt-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">
              <ServerStackIcon className="h-3.5 w-3.5" />
              Sources
            </h3>
            <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
              <p className="px-3 py-2 text-sm text-zinc-600">Sources coming in Phase 5.</p>
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
