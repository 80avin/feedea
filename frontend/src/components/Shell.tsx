import { useSession } from "../auth/useSession";
import DesktopLayout from "../layout/DesktopLayout";
import MobileLayout from "../layout/MobileLayout";
import { ErrorState } from "./Feedback";

export default function Shell() {
  const { session, loading, isError, error, retry } = useSession();

  if (isError) {
    return (
      <main className="flex min-h-screen items-center justify-center bg-app-bg text-app-text">
        <ErrorState error={error} onRetry={retry} />
      </main>
    );
  }

  if (loading || !session?.authenticated) {
    return <main className="flex min-h-screen items-center justify-center bg-app-bg text-app-text">Loading...</main>;
  }

  return (
    <div className="bg-app-bg text-app-text">
      <DesktopLayout />
      <MobileLayout />
    </div>
  );
}
