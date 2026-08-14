import { Outlet } from "react-router";
import { Button } from "@heroui/react";
import { useSession } from "../auth/useSession";

export default function Shell() {
  const { session, loading, logout } = useSession();

  if (loading || !session?.authenticated) {
    return <main className="flex min-h-screen items-center justify-center bg-zinc-950 text-zinc-100">Loading...</main>;
  }

  return (
    <div className="flex min-h-screen flex-col bg-zinc-950 text-zinc-100">
      <header className="flex items-center justify-between border-b border-zinc-800 px-4 py-3">
        <span className="text-lg font-bold tracking-tight">rssea</span>
        <Button variant="outline" size="sm" onPress={logout}>
          Logout
        </Button>
      </header>
      <main className="flex-1">
        <Outlet />
      </main>
    </div>
  );
}
