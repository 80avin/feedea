import type { ComponentType } from "react";
import { NavLink } from "react-router";
import {
  BookmarkIcon,
  Cog6ToothIcon,
  RssIcon,
  ServerStackIcon,
  Squares2X2Icon,
} from "@heroicons/react/24/outline";
import clsx from "clsx";

type Icon = ComponentType<{ className?: string }>;

interface Item {
  to: string;
  label: string;
  icon: Icon;
  end?: boolean;
}

const items: Item[] = [
  { to: "/feeds", label: "Feed", icon: RssIcon },
  { to: "/sources", label: "Sources", icon: ServerStackIcon },
  { to: "/", label: "Overview", icon: Squares2X2Icon, end: true },
  { to: "/saved", label: "Saved", icon: BookmarkIcon },
  { to: "/settings", label: "Settings", icon: Cog6ToothIcon },
];

export default function BottomNav() {
  return (
    <nav className="flex border-t border-zinc-800 bg-zinc-950">
      {items.map(({ to, label, icon: Icon, end }) => (
        <NavLink
          key={to}
          to={to}
          end={end}
          className={({ isActive }) =>
            clsx(
              "flex flex-1 flex-col items-center gap-0.5 py-2 text-[11px] font-medium",
              isActive ? "text-zinc-100" : "text-zinc-500",
            )
          }
        >
          <Icon className="h-5 w-5" />
          {label}
        </NavLink>
      ))}
    </nav>
  );
}
