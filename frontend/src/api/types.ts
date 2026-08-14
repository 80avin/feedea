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
