export interface SessionInfo {
  authenticated: boolean;
  version: string;
  setup_required: boolean;
}

export interface ErrorEnvelope {
  error: {
    code: string;
    message: string;
  };
}

export interface FeedSummary {
  id: string;
  title: string;
  website: string | null;
  feed_url: string | null;
  icon_url: string | null;
  category_id: string;
  unread_count: number;
  error_count: number;
  error_message: string | null;
}

export interface Headline {
  id: string;
  title: string | null;
  feed_id: string;
  feed_title: string | null;
  url: string | null;
  date: string;
  summary: string | null;
  thumbnail_url: string | null;
  unread: boolean;
  marked: boolean;
  note?: string;
  tags?: string[];
}

export interface CategoryCard {
  category_id: string;
  name: string;
  total_count: number;
  unread_count: number;
  items: Headline[];
}

export interface OverviewResponse {
  cards: CategoryCard[];
  all: {
    total_count: number;
    unread_count: number;
  };
}

export interface SettingsStats {
  feeds: number;
  articles: number;
  unread: number;
  database_size_bytes: number;
  last_sync: string;
}

export interface Settings {
  theme: string | null;
  accent: string | null;
  sync_interval_minutes: number;
  keep_articles_days: number | null;
  stats: SettingsStats;
}

export interface ArticleDetail {
  id: string;
  title: string | null;
  author: string | null;
  feed_id: string;
  feed_title: string | null;
  url: string | null;
  date: string;
  html: string | null;
  summary: string | null;
  unread: boolean;
  marked: boolean;
  thumbnail_url: string | null;
  plain_text: string | null;
  note: string | null;
  tags: string[];
}

export interface CategoryNode {
  category_id: string;
  name: string;
  parent_id: string;
  unread_count: number;
  children: CategoryNode[];
}

export interface CategoriesResponse {
  categories: CategoryNode[];
}

export interface SourceGroup {
  category_id: string;
  category_name: string;
  feeds: FeedSummary[];
}

export interface SourcesResponse {
  groups: SourceGroup[];
}

export interface DiscoverAlternative {
  label: string;
  url: string;
}

export interface DiscoverResponse {
  title: string | null;
  feed_url: string | null;
  alternatives: DiscoverAlternative[];
}

export interface MonthGroup {
  month: string;
  items: Headline[];
}

export interface SavedResponse {
  months: MonthGroup[];
  total: number;
}

export interface TagsResponse {
  tags: string[];
}

export interface SuggestionsResponse {
  suggestions: Headline[];
}

export interface ArticleQueryParams {
  feed?: string;
  category?: string;
  saved?: string;
  unread?: string;
  tag?: string;
  search?: string;
}

export interface OpmlEntry {
  index: number;
  title: string;
  url: string;
  category: string[];
}

export interface OpmlExistingFeed {
  id: string;
  title: string;
  url: string | null;
  website: string | null;
  category: string[];
}

export interface OpmlConflict {
  key: number;
  kind: "same-feed" | "intra-file";
  opml: OpmlEntry;
  matches: OpmlExistingFeed[];
  occurrences?: number;
}

export interface OpmlResolution {
  key: number;
  action: "keep-new" | "keep-existing" | "skip";
  keep_existing_feed_id?: string;
}

export interface ImportOpmlResponse {
  status: "imported" | "conflicts";
  added?: number;
  updated?: number;
  skipped?: number;
  migrated?: number;
  conflicts_resolved?: number;
  conflicts?: OpmlConflict[];
  stats?: { new: number; exact_duplicates: number };
}
