import { useSession } from "../auth/useSession";

export default function Home() {
  const { session } = useSession();

  return (
    <div className="flex min-h-full flex-col items-center justify-center gap-2 p-6">
      <h1 className="text-2xl font-bold">Home</h1>
      <p className="text-sm text-zinc-400">You are logged in{session?.version ? ` (v${session.version})` : ""}.</p>
    </div>
  );
}
