import { useSession } from "../auth/useSession";
import DesktopLayout from "../layout/DesktopLayout";
import MobileLayout from "../layout/MobileLayout";

export default function Shell() {
  const { session, loading } = useSession();

  if (loading || !session?.authenticated) {
    return <main className="flex min-h-screen items-center justify-center bg-zinc-950 text-zinc-100">Loading...</main>;
  }

  return (
    <div className="bg-zinc-950 text-zinc-100">
      <DesktopLayout />
      <MobileLayout />
    </div>
  );
}
