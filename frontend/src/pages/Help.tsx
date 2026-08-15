import { QuestionMarkCircleIcon } from "@heroicons/react/24/outline";

const REPO_URL = "https://github.com/80avin/feedea";
const ISSUES_URL = "https://github.com/80avin/feedea/issues";

export default function Help() {
  return (
    <div className="flex h-full flex-col p-4">
      <div className="flex items-center gap-2">
        <QuestionMarkCircleIcon className="h-5 w-5 text-app-text-muted" />
        <h2 className="text-lg font-semibold">Help</h2>
      </div>

      <div className="mt-4 flex-1 space-y-6 overflow-y-auto text-sm">
        <section className="flex flex-col gap-3 rounded-lg border border-app-border p-4">
          <h3 className="font-semibold">Getting started</h3>
          <ul className="list-disc space-y-1.5 pl-5 text-app-text-muted">
            <li>
              <span className="text-app-text">Add sources</span> from the Sources view
              (sidebar on desktop, bottom nav on mobile). Paste a feed URL, or import an
              OPML file.
            </li>
            <li>
              <span className="text-app-text">Refresh feeds</span> with the refresh button
              on a source, or let the automatic sync interval (Settings) do it for you.
            </li>
            <li>
              <span className="text-app-text">Save articles</span> to keep them in the Saved
              view. You can add a note and tags, and filter by tag from the timeline.
            </li>
          </ul>
        </section>

        <section className="flex flex-col gap-3 rounded-lg border border-app-border p-4">
          <h3 className="font-semibold">Keyboard &amp; interface tips</h3>
          <ul className="list-disc space-y-1.5 pl-5 text-app-text-muted">
            <li>
              Click an article to open it in the reader. The actions row lets you mark it
              read/unread, save it, open the original, or share it.
            </li>
            <li>
              Use <span className="text-app-text">Search</span> at the top of the timeline
              to find articles; suggestions appear as you type.
            </li>
            <li>
              In the reader, external links open in a new tab, and hovering a link or image
              shows its destination URL.
            </li>
          </ul>
        </section>

        <section className="flex flex-col gap-3 rounded-lg border border-app-border p-4">
          <h3 className="font-semibold">About</h3>
          <p className="text-app-text-muted">feedea is a self-hosted RSS feed aggregator.</p>
          <div className="flex flex-wrap gap-2">
            <a
              href={REPO_URL}
              target="_blank"
              rel="noreferrer"
              className="rounded-md border border-app-border-strong px-3 py-1.5 text-sm text-app-text-2 hover:border-app-border hover:text-app-text"
            >
              Repository
            </a>
            <a
              href={ISSUES_URL}
              target="_blank"
              rel="noreferrer"
              className="rounded-md border border-app-border-strong px-3 py-1.5 text-sm text-app-text-2 hover:border-app-border hover:text-app-text"
            >
              Report an issue
            </a>
          </div>
        </section>
      </div>
    </div>
  );
}
