import { Button } from "@heroui/react";
import { ArrowRightStartOnRectangleIcon } from "@heroicons/react/24/outline";
import { useSession } from "../auth/useSession";

export default function Settings() {
  const { logout } = useSession();

  return (
    <div className="flex h-full flex-col p-4">
      <h2 className="text-lg font-semibold">Settings</h2>
      <p className="text-sm text-zinc-400">Settings coming in Phase 5.</p>
      <div className="flex-1" />
      <Button variant="ghost" size="sm" fullWidth onPress={logout} className="justify-start">
        <ArrowRightStartOnRectangleIcon className="h-4 w-4" />
        Log out
      </Button>
    </div>
  );
}
