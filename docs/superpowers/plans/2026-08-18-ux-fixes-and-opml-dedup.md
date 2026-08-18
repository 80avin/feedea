# feedea UX Fixes + OPML Dedup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the 7 approved UX/import improvements from the design spec: list markers, filter-preserving article links, OPML dedup with conflict resolution, clickable source names, article + sidebar context menus, filter indicators, and PWA install/update UX.

**Architecture:** Backend is a Rust axum server wrapping the news-flash crate (SQLite). OPML dedup lives in a new feedea-side module (`src/engine/opml_import.rs`) that parses/classifies the OPML and drives news-flash's existing `import_opml` with a cleaned document, plus a small direct-SQLite migration step. Frontend is React 19 + Vite + Tailwind v4 + react-router 8; all changes are URL-query-param driven (no new state store).

**Tech Stack:** Rust 2024, axum 0.8, news-flash 3.2.0, `opml` 1.1.6, rusqlite (bundled); React 19, react-router 8, TanStack Query 5, vite-plugin-pwa 1.3.0, Tailwind CSS 4, bun.

## Global Constraints

- Rust edition 2024; run `cargo fmt` before every commit. Backend verification: `cargo test` (must pass) and `cargo build`.
- Frontend verification: `bun run typecheck` and `bun run lint` must pass; `bun run build` must succeed. Run from `frontend/`.
- Full verification (final task): `make test` from repo root (builds frontend, runs cargo test, typecheck, lint).
- The design spec is `docs/superpowers/specs/2026-08-18-ux-fixes-and-opml-dedup-design.md`. Read it before starting. Do not deviate from the JSON contract below.
- news-flash feed id == the raw feed URL string (`FeedID::new(url)`, verified in `news-flash-3.2.0/src/models/feed.rs:33,91` and `src/util/opml/mod.rs:185`). `insert_feeds` uses `replace_into` keyed on that id. This is why URL *variants* (http/https, trailing slash) are real duplicates.
- No comments in code unless they clarify non-obvious logic.

### OPML import JSON contract (must match exactly between backend Task 2 and frontend Task 8)

```
OpmlEntry    = { index: number, title: string, url: string, category: string }
ExistingFeed = { id: string, title: string, url: string | null, website: string | null, category: string }
Conflict     = { key: number, kind: "url-identical" | "url-variant" | "intra-file",
                 opml: OpmlEntry, matches: ExistingFeed[] }
Resolution   = { key: number, action: "keep-new" | "keep-existing" | "skip",
                 keep_existing_feed_id?: string }
```
- Request: `POST /api/sources/import-opml` body `{ opml: string, resolutions?: Resolution[] }`.
- No `resolutions` + conflicts exist → response `{ status: "conflicts", conflicts: Conflict[], stats: { new: number, exact_duplicates: number } }`, **no DB writes**.
- No `resolutions` + no conflicts → import, respond `{ status: "imported", added, skipped, migrated, conflicts_resolved }` (added/skipped are counts).
- With `resolutions` → import + apply resolutions, respond same `status: "imported"` shape.
- `kind` semantics:
  - `url-identical`: OPML xml_url string equals an existing feed's id/feed_url. `keep-new` = rename existing feed to the OPML title (+ move category if given and different). No migration, no removal.
  - `url-variant`: normalized URL matches an existing feed but the string differs. `keep-new` = import the new feed (new id = xml_url string), migrate the existing feed's articles (`UPDATE articles SET feed_id = new WHERE feed_id = old`), then remove the old feed.
  - `intra-file`: same normalized URL appears earlier in the file. `keep-new` = this occurrence's title/category win in the cleaned OPML; default `keep-existing` = first occurrence wins.
- Feeds with id prefix `__file__:` are synthetic intra-file matches (never migrate/remove them).
- URL normalization: lowercase scheme + host, keep explicit port, strip trailing `/`, strip fragment, keep query. Note: `http` and `https` remain **different** schemes (only the scheme string is lowercased).
- Classification precedence per entry: existing-feed match wins over intra-file. `url-identical` (raw xml_url == an existing id) → `url-variant` (normalized match) → `intra-file` (duplicate of an earlier file URL, no existing match) → `new`.
- Existing feeds in the toplevel category (`category_id == "NewsFlash.Toplevel"`) are exposed to the frontend with `category: ""`, matching an OPML feed with no category outline, so exact duplicates in the toplevel are detected as skips rather than conflicts.

---

## Task 1: Backend — OPML parse/classify module

**Files:**
- Create: `src/engine/opml_import.rs`
- Modify: `src/engine/mod.rs` (add `pub mod opml_import;` in the module list at lines 20-22)
- Modify: `Cargo.toml` (add `opml = "1.1"` under `[dependencies]`)

**Interfaces:**
- Produces (consumed by Task 2 and the API):
  - `pub struct OpmlEntry { pub index: usize, pub title: String, pub url: String, pub category: String }` (derive `Debug, Clone, Serialize`)
  - `pub struct ExistingFeed { pub id: String, pub title: String, pub url: Option<String>, pub website: Option<String>, pub category: String }` (derive `Debug, Clone, Serialize`)
  - `pub enum ConflictKind { UrlIdentical, UrlVariant, IntraFile }` (derive `Debug, Clone, Copy, PartialEq, Eq, Serialize`, serde `#[serde(rename_all = "kebab-case")]`)
  - `pub struct Conflict { pub key: usize, pub kind: ConflictKind, pub opml: OpmlEntry, pub matches: Vec<ExistingFeed> }` (derive `Debug, Clone, Serialize`)
  - `pub struct Classification { pub conflicts: Vec<Conflict>, pub new_count: usize, pub exact_duplicates: usize, pub skipped: std::collections::HashSet<usize> }`
  - `pub fn parse_entries(opml_str: &str) -> anyhow::Result<Vec<OpmlEntry>>`
  - `pub fn normalize_url(url: &str) -> Option<String>`
  - `pub fn classify(entries: &[OpmlEntry], existing: &[ExistingFeed]) -> Classification`

- [ ] **Step 1: Add dependency**

In `Cargo.toml` `[dependencies]` add `opml = "1.1"`.

Run: `cargo build`
Expected: builds (opml 1.1.6 is already in the lockfile via news-flash).

- [ ] **Step 2: Write the failing unit tests**

Add this test module at the bottom of `src/engine/opml_import.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const OPML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example.com/feed.xml"/>
      <outline text="Feed A Again" title="Feed A Again" type="rss" xmlUrl="https://example.com/feed.xml/"/>
    </outline>
    <outline text="Feed B" title="Feed B" type="rss" xmlUrl="http://example.org/b/"/>
  </body>
</opml>"#;

    fn existing() -> Vec<ExistingFeed> {
        vec![
            ExistingFeed {
                id: "https://example.com/feed.xml".to_string(),
                title: "Feed A".to_string(),
                url: Some("https://example.com/feed.xml".to_string()),
                website: Some("https://example.com".to_string()),
                category: "Tech".to_string(),
            },
            ExistingFeed {
                id: "http://example.org/b".to_string(),
                title: "Feed B".to_string(),
                url: Some("http://example.org/b".to_string()),
                website: None,
                category: "".to_string(),
            },
        ]
    }

    #[test]
    fn parses_feeds_with_category_path_and_indices() {
        let entries = parse_entries(OPML).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].title, "Feed A");
        assert_eq!(entries[0].url, "https://example.com/feed.xml");
        assert_eq!(entries[0].category, "Tech");
        assert_eq!(entries[1].index, 1);
        assert_eq!(entries[1].category, "Tech");
        assert_eq!(entries[2].index, 2);
        assert_eq!(entries[2].category, "");
    }

    #[test]
    fn normalize_url_strips_trailing_slash_and_fragment() {
        assert_eq!(
            normalize_url("HTTP://Example.COM:443/feed.xml/"),
            normalize_url("http://example.com:443/feed.xml#frag")
        );
        assert_eq!(normalize_url("https://example.com"), Some("https://example.com".into()));
        assert_eq!(normalize_url("not a url"), None);
    }

    #[test]
    fn classifies_url_identical_and_url_variant_conflicts_and_new() {
        // Feed A exists with identical raw url -> url-identical conflict (title matches, so nothing)
        // Use a variant title to force a conflict.
        let mut existing = existing();
        existing[0].title = "Renamed".to_string();
        let entries = parse_entries(OPML).unwrap();
        let c = classify(&entries, &existing);
        assert_eq!(c.new_count, 0);
        assert_eq!(c.exact_duplicates, 0);
        // entry 0: url-identical (id == url, title differs)
        // entry 1: url-variant (trailing-slash variant of Feed A's url)
        // entry 2: url-variant (trailing-slash variant of Feed B's url)
        let kinds: Vec<ConflictKind> = c.conflicts.iter().map(|x| x.kind).collect();
        assert!(kinds.contains(&ConflictKind::UrlIdentical));
        assert!(kinds.contains(&ConflictKind::UrlVariant));
        let b = c.conflicts.iter().find(|x| x.key == 2).unwrap();
        assert_eq!(b.matches[0].id, "http://example.org/b");
        assert_eq!(b.kind, ConflictKind::UrlVariant);
    }

    #[test]
    fn classifies_intra_file_conflict_when_title_differs() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="First" title="First" type="rss" xmlUrl="https://example.com/dup"/>
    <outline text="Second" title="Second" type="rss" xmlUrl="https://example.com/dup/"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let c = classify(&entries, &[]);
        assert_eq!(c.new_count, 1);
        let conflict = c.conflicts.iter().find(|x| x.key == 1).unwrap();
        assert_eq!(conflict.kind, ConflictKind::IntraFile);
        assert_eq!(conflict.matches.len(), 1);
        assert_eq!(conflict.matches[0].id, "__file__:0");
        assert_eq!(conflict.matches[0].title, "First");
        assert_eq!(c.exact_duplicates, 0);
    }

    #[test]
    fn classifies_exact_duplicate_as_skipped() {
        let entries = parse_entries(OPML).unwrap();
        let c = classify(&entries, &existing());
        // entry 0 matches Feed A exactly -> skipped
        assert_eq!(c.exact_duplicates, 1);
        assert!(c.skipped.contains(&0));
        // entry 1: url-variant (trailing-slash variant of Feed A's url, matches Feed A)
        // entry 2: url-variant (trailing-slash variant of Feed B's url, matches Feed B)
        assert_eq!(c.conflicts.len(), 2);
        assert_eq!(c.new_count, 0);
    }

    #[test]
    fn intra_file_exact_duplicate_is_skipped() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Same" title="Same" type="rss" xmlUrl="https://example.com/x"/>
    <outline text="Same" title="Same" type="rss" xmlUrl="https://example.com/x"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let c = classify(&entries, &[]);
        assert_eq!(c.exact_duplicates, 1);
        assert!(c.skipped.contains(&1));
        assert_eq!(c.new_count, 1);
    }

    #[test]
    fn unparseable_entry_url_is_new_not_a_conflict() {
        // Feed B in existing() has website: None. A malformed/scheme-less entry url
        // normalizes to None and must NOT match it via the None == None path.
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Bogus" title="Bogus" type="rss" xmlUrl="www.example.com/feed.xml"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let c = classify(&entries, &existing());
        assert_eq!(c.new_count, 1);
        assert_eq!(c.conflicts.len(), 0);
        assert_eq!(c.exact_duplicates, 0);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test engine::opml_import`
Expected: compile error — module/file doesn't exist yet.

- [ ] **Step 4: Implement the module**

Write `src/engine/opml_import.rs` implementing the exact logic below.

```rust
use std::collections::{HashMap, HashSet};

use opml::{OPML, Outline};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OpmlEntry {
    pub index: usize,
    pub title: String,
    pub url: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExistingFeed {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub website: Option<String>,
    pub category: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    UrlIdentical,
    UrlVariant,
    IntraFile,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub key: usize,
    pub kind: ConflictKind,
    pub opml: OpmlEntry,
    pub matches: Vec<ExistingFeed>,
}

#[derive(Debug, Clone)]
pub struct Classification {
    pub conflicts: Vec<Conflict>,
    pub new_count: usize,
    pub exact_duplicates: usize,
    pub skipped: HashSet<usize>,
}

fn collect(outlines: &[Outline], category: &str, index: &mut usize, out: &mut Vec<OpmlEntry>) {
    for outline in outlines {
        if let Some(xml_url) = &outline.xml_url {
            let title = outline
                .title
                .clone()
                .filter(|t| !t.is_empty())
                .or_else(|| Some(outline.text.clone()))
                .unwrap_or_else(|| "No Title".to_string());
            out.push(OpmlEntry {
                index: *index,
                title,
                url: xml_url.clone(),
                category: category.to_string(),
            });
            *index += 1;
        } else {
            let title = outline
                .title
                .clone()
                .filter(|t| !t.is_empty())
                .or_else(|| Some(outline.text.clone()))
                .unwrap_or_default();
            let child_category = if title.is_empty() { category.to_string() } else { title };
            collect(&outline.outlines, &child_category, index, out);
        }
    }
}

pub fn parse_entries(opml_str: &str) -> anyhow::Result<Vec<OpmlEntry>> {
    let doc = OPML::from_str(opml_str).map_err(|e| anyhow::anyhow!("invalid opml: {e}"))?;
    let mut entries = Vec::new();
    let mut index = 0;
    collect(&doc.body.outlines, "", &mut index, &mut entries);
    Ok(entries)
}

pub fn normalize_url(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw).ok()?;
    let scheme = parsed.scheme().to_lowercase();
    let host = parsed.host_str()?.to_lowercase();
    let mut normalized = format!("{scheme}://{host}");
    if let Some(port) = parsed.port() {
        normalized.push_str(&format!(":{port}"));
    }
    let path = parsed.path().trim_end_matches('/');
    if !path.is_empty() {
        normalized.push_str(path);
    }
    if let Some(query) = parsed.query() {
        normalized.push('?');
        normalized.push_str(query);
    }
    Some(normalized)
}

pub fn classify(entries: &[OpmlEntry], existing: &[ExistingFeed]) -> Classification {
    let mut conflicts = Vec::new();
    let mut skipped = HashSet::new();
    let mut new_count = 0;
    let mut first_by_url: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        let norm = normalize_url(&entry.url);
        let is_first = norm.as_ref().map(|n| !first_by_url.contains_key(n)).unwrap_or(true);

        let matches: Vec<ExistingFeed> = existing
            .iter()
            .filter(|f| {
                // never let normalize_url == None on the left match a None url/website:
                // an unparseable entry url must not match website-less existing feeds
                f.id == entry.url
                    || norm.as_ref().is_some_and(|n| {
                        f.url.as_deref().and_then(normalize_url).as_ref() == Some(n)
                            || f.website.as_deref().and_then(normalize_url).as_ref() == Some(n)
                    })
            })
            .cloned()
            .collect();

        let id_match = matches.iter().find(|f| f.id == entry.url);

        if let Some(idm) = id_match {
            if idm.title == entry.title && idm.category == entry.category {
                skipped.insert(entry.index);
            } else {
                conflicts.push(Conflict {
                    key: entry.index,
                    kind: ConflictKind::UrlIdentical,
                    opml: entry.clone(),
                    matches: matches.clone(),
                });
            }
        } else if !matches.is_empty() {
            conflicts.push(Conflict {
                key: entry.index,
                kind: ConflictKind::UrlVariant,
                opml: entry.clone(),
                matches: matches.clone(),
            });
        } else if !is_first {
            let n = norm.as_ref().unwrap();
            let first_index = first_by_url[n];
            let first_entry = entries.iter().find(|e| e.index == first_index).unwrap();
            let synthetic = ExistingFeed {
                id: format!("__file__:{first_index}"),
                title: first_entry.title.clone(),
                url: Some(first_entry.url.clone()),
                website: None,
                category: first_entry.category.clone(),
            };
            if synthetic.title == entry.title && synthetic.category == entry.category {
                skipped.insert(entry.index);
            } else {
                conflicts.push(Conflict {
                    key: entry.index,
                    kind: ConflictKind::IntraFile,
                    opml: entry.clone(),
                    matches: vec![synthetic],
                });
            }
        } else {
            new_count += 1;
        }

        if let Some(n) = norm {
            first_by_url.entry(n).or_insert(entry.index);
        }
    }

    Classification {
        conflicts,
        new_count,
        exact_duplicates: skipped.len(),
        skipped,
    }
}
```

- [ ] **Step 5: Register the module**

In `src/engine/mod.rs`, add `pub mod opml_import;` alongside the existing `pub mod content; pub mod queries; pub mod sync;` (around line 20-22).

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test engine::opml_import`
Expected: all 5 tests pass.

- [ ] **Step 7: Format and commit**

Run: `cargo fmt`
Then:
```bash
git add Cargo.toml Cargo.lock src/engine/opml_import.rs src/engine/mod.rs
git commit -m "feat(backend): opml parse and duplicate classification"
```

---

## Task 2: Backend — two-phase import endpoint + migration

**Files:**
- Modify: `src/api/sources.rs` (import_opml handler + request struct)
- Modify: `src/engine/opml_import.rs` (add `migrate_feed_articles`, `build_cleaned_opml`)
- Modify: `tests/sources.rs` (update `import_and_export_opml`; add dedup tests)

**Interfaces:**
- Consumes: Task 1's `OpmlEntry`, `ExistingFeed`, `Conflict`, `ConflictKind`, `Classification`, `parse_entries`, `normalize_url`, `classify`.
- Produces:
  - `pub struct Resolution { pub key: usize, pub action: ResolutionAction, pub keep_existing_feed_id: Option<String> }` (derive `Deserialize`)
  - `pub enum ResolutionAction { KeepNew, KeepExisting, Skip }` (derive `Deserialize, PartialEq, Eq`, serde `#[serde(rename_all = "kebab-case")]`)
  - `pub fn migrate_feed_articles(db_path: &std::path::Path, from_feed_id: &str, to_feed_id: &str) -> anyhow::Result<u64>`
  - `pub fn build_cleaned_opml(opml_str: &str, entries: &[OpmlEntry], classification: &Classification, resolutions: &[Resolution]) -> anyhow::Result<(String, usize)>` — returns (cleaned opml, number of feed outlines in it = `added` count).

- [ ] **Step 1: Write the failing classification-helper test for cleaned-opml builder**

Add to the `#[cfg(test)] mod tests` in `src/engine/opml_import.rs`:

```rust
#[test]
fn build_cleaned_opml_drops_exact_dups_keeps_resolved() {
    use crate::engine::opml_import::*;
    let opml = OPML.to_string();
    let entries = parse_entries(&opml).unwrap();
    let classification = classify(&entries, &existing());
    // entry 0 is an exact duplicate (skipped). Entries 1 and 2 are url-variant conflicts.
    let resolutions = vec![
        Resolution { key: 1, action: ResolutionAction::KeepExisting, keep_existing_feed_id: None },
        Resolution { key: 2, action: ResolutionAction::KeepNew, keep_existing_feed_id: None },
    ];
    let (cleaned, added) = build_cleaned_opml(&opml, &entries, &classification, &resolutions).unwrap();
    assert_eq!(added, 1); // entry 1 dropped (keep-existing), entry 2 kept (keep-new), entry 0 skipped
    let cleaned_entries = parse_entries(&cleaned).unwrap();
    assert_eq!(cleaned_entries.len(), 1);
    assert!(cleaned_entries.iter().all(|e| e.url != "https://example.com/feed.xml"));
    assert!(cleaned_entries.iter().any(|e| e.url == "http://example.org/b/"));
}

#[test]
fn build_cleaned_opml_intra_file_keep_new_prefers_later_occurrence() {
    use crate::engine::opml_import::*;
    let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="First" title="First" type="rss" xmlUrl="https://example.com/dup"/>
    <outline text="Second" title="Second" type="rss" xmlUrl="https://example.com/dup/"/>
  </body>
</opml>"#;
    let entries = parse_entries(opml).unwrap();
    let classification = classify(&entries, &[]);
    // intra-file conflict at key 1; keep-new -> the later occurrence wins that url
    let resolutions = vec![
        Resolution { key: 1, action: ResolutionAction::KeepNew, keep_existing_feed_id: None },
    ];
    let (cleaned, added) = build_cleaned_opml(opml, &entries, &classification, &resolutions).unwrap();
    assert_eq!(added, 1);
    let cleaned_entries = parse_entries(&cleaned).unwrap();
    assert_eq!(cleaned_entries.len(), 1);
    assert_eq!(cleaned_entries[0].title, "Second");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test build_cleaned_opml`
Expected: FAIL (no `Resolution`, `build_cleaned_opml` yet).

- [ ] **Step 3: Implement `Resolution`, `ResolutionAction`, `build_cleaned_opml`, `migrate_feed_articles`**

Append to `src/engine/opml_import.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ResolutionAction {
    KeepNew,
    KeepExisting,
    Skip,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Resolution {
    pub key: usize,
    pub action: ResolutionAction,
    pub keep_existing_feed_id: Option<String>,
}

pub fn migrate_feed_articles(db_path: &std::path::Path, from_feed_id: &str, to_feed_id: &str) -> anyhow::Result<u64> {
    if from_feed_id == to_feed_id {
        return Ok(0);
    }
    let conn = rusqlite::Connection::open(db_path)?;
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    let n = conn.execute(
        "UPDATE articles SET feed_id = ?1 WHERE feed_id = ?2",
        rusqlite::params![to_feed_id, from_feed_id],
    )?;
    Ok(n as u64)
}

fn keep_new_keys(resolutions: &[Resolution]) -> HashSet<usize> {
    resolutions
        .iter()
        .filter(|r| r.action == ResolutionAction::KeepNew)
        .map(|r| r.key)
        .collect()
}

fn filter_outlines(outlines: &mut Vec<Outline>, index: &mut usize, keep: &HashSet<usize>) {
    outlines.retain(|outline| {
        if outline.xml_url.is_some() {
            let idx = *index;
            *index += 1;
            keep.contains(&idx)
        } else {
            true
        }
    });
    for outline in outlines.iter_mut() {
        if outline.xml_url.is_none() {
            filter_outlines(&mut outline.outlines, index, keep);
        }
    }
}

pub fn build_cleaned_opml(
    opml_str: &str,
    entries: &[OpmlEntry],
    classification: &Classification,
    resolutions: &[Resolution],
) -> anyhow::Result<(String, usize)> {
    let mut doc = OPML::from_str(opml_str).map_err(|e| anyhow::anyhow!("invalid opml: {e}"))?;
    let keep_new = keep_new_keys(resolutions);
    let conflict_by_key: HashMap<usize, &Conflict> =
        classification.conflicts.iter().map(|c| (c.key, c)).collect();

    // For intra-file conflicts resolved to keep-new, the later occurrence wins that url.
    let mut intra_winner: HashMap<String, usize> = HashMap::new();
    for (key, conflict) in &conflict_by_key {
        if conflict.kind == ConflictKind::IntraFile && keep_new.contains(key) {
            intra_winner.insert(conflict.opml.url.clone(), conflict.opml.index);
        }
    }

    let mut keep = HashSet::new();
    for entry in entries {
        if classification.skipped.contains(&entry.index) {
            continue;
        }
        if let Some(conflict) = conflict_by_key.get(&entry.index) {
            match conflict.kind {
                ConflictKind::UrlVariant => {
                    if keep_new.contains(&entry.index) {
                        keep.insert(entry.index);
                    }
                }
                ConflictKind::UrlIdentical => {
                    // handled by rename/move in the handler; never imported via opml
                }
                ConflictKind::IntraFile => {
                    let winner = intra_winner.get(&entry.url).copied();
                    if winner == Some(entry.index) {
                        keep.insert(entry.index);
                    }
                }
            }
        } else {
            // brand-new feed
            let winner = intra_winner.get(&entry.url).copied();
            if winner.map_or(true, |w| w == entry.index) {
                keep.insert(entry.index);
            }
        }
    }

    let mut index = 0usize;
    filter_outlines(&mut doc.body.outlines, &mut index, &keep);
    let cleaned = doc.to_string().map_err(|e| anyhow::anyhow!("serialize opml: {e}"))?;
    Ok((cleaned, keep.len()))
}
```

Note: `url` and `Outline` imports at the top of the file must include `std::collections::{HashMap, HashSet}` (already added in Task 1) and `opml::Outline`.

- [ ] **Step 4: Run the new test to verify it passes**

Run: `cargo test build_cleaned_opml`
Expected: PASS.

- [ ] **Step 5: Write the failing endpoint tests**

Update `tests/sources.rs`:

(a) Change `import_and_export_opml` (line ~493) from `assert_eq!(value["imported"], true);` to `assert_eq!(value["status"], "imported");`.

(b) Append these tests to `tests/sources.rs`:

```rust
#[tokio::test]
async fn opml_exact_duplicate_import_is_skipped() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);
    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Test Feed" title="Test Feed" type="rss" xmlUrl="{feed_url}"/>
  </body>
</opml>"#
    );
    let body = serde_json::json!({ "opml": opml }).to_string();
    let first = app.clone().oneshot(
        Request::builder().method("POST").uri("/api/sources/import-opml")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(body.clone())).unwrap(),
    ).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_val: serde_json::Value =
        serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(first_val["status"], "imported");
    assert_eq!(first_val["added"], 1);

    let second = app.clone().oneshot(
        Request::builder().method("POST").uri("/api/sources/import-opml")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();
    let second_val: serde_json::Value =
        serde_json::from_slice(&second.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(second_val["status"], "imported");
    assert_eq!(second_val["added"], 0);
    assert_eq!(second_val["skipped"], 1);

    let groups = get_groups(&app, &cookie).await;
    let count: usize = groups.iter().flat_map(|g| g["feeds"].as_array().unwrap()).count();
    assert_eq!(count, 1, "importing same opml twice must not duplicate the feed");
}

#[tokio::test]
async fn opml_url_variant_conflict_keeps_new_and_migrates_articles() {
    let (feed_url, app, _server, _db) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add = app.clone().oneshot(
        Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(serde_json::json!({ "url": feed_url, "title": "Old Title" }).to_string())).unwrap(),
    ).await.unwrap();
    assert_eq!(add.status(), StatusCode::OK);

    let variant_url = format!("{feed_url}/");
    let opml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="New Title" title="New Title" type="rss" xmlUrl="{variant_url}"/>
  </body>
</opml>"#
    );

    // phase 1 -> conflicts
    let body = serde_json::json!({ "opml": opml }).to_string();
    let resp = app.clone().oneshot(
        Request::builder().method("POST").uri("/api/sources/import-opml")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(body.clone())).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let val: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val["status"], "conflicts");
    let conflict = &val["conflicts"][0];
    assert_eq!(conflict["kind"], "url-variant");
    let key = conflict["key"].as_u64().unwrap() as usize;

    // phase 2 -> keep new
    let resolutions = serde_json::json!([{ "key": key, "action": "keep-new" }]);
    let body2 = serde_json::json!({ "opml": opml, "resolutions": resolutions }).to_string();
    let resp2 = app.clone().oneshot(
        Request::builder().method("POST").uri("/api/sources/import-opml")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(body2)).unwrap(),
    ).await.unwrap();
    let val2: serde_json::Value =
        serde_json::from_slice(&resp2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(val2["status"], "imported");

    // exactly one feed remains, with the new title, and articles survived the migration
    let groups = get_groups(&app, &cookie).await;
    let feeds: Vec<&serde_json::Value> = groups.iter().flat_map(|g| g["feeds"].as_array().unwrap()).collect();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0]["title"], "New Title");
    assert_eq!(feeds[0]["feed_url"], variant_url);

    let articles = app.clone().oneshot(
        Request::builder().method("GET").uri("/api/articles")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();
    let body_articles = articles.into_body().collect().await.unwrap().to_bytes();
    let val_articles: serde_json::Value = serde_json::from_slice(&body_articles).unwrap();
    let items = val_articles.as_array().unwrap();
    assert_eq!(items.len(), 2, "articles from the old feed must be migrated to the new feed");
}
```

- [ ] **Step 6: Run the new tests to verify they fail**

Run: `cargo test --test sources opml_`
Expected: FAIL (import handler doesn't yet implement the contract).

- [ ] **Step 7: Implement the endpoint**

In `src/api/sources.rs`:

(a) Update imports at the top to include:
```rust
use std::collections::HashMap;
use crate::engine::opml_import::{self, Conflict, ConflictKind, ExistingFeed, Resolution, ResolutionAction};
```

(b) Update `ImportOpmlRequest`:
```rust
#[derive(Deserialize)]
pub struct ImportOpmlRequest {
    pub opml: String,
    pub resolutions: Option<Vec<Resolution>>,
}
```

(c) Replace the `import_opml` handler (currently `src/api/sources.rs:148-154`) with:

```rust
pub async fn import_opml(
    State(state): State<AppState>,
    Json(req): Json<ImportOpmlRequest>,
) -> ApiResult<Json<Value>> {
    let feeds = state.engine.get_feeds().await?;
    let (categories, _) = state.engine.get_categories().await?;
    let name_by_id: HashMap<String, String> = categories
        .into_iter()
        .map(|c| (c.category_id.as_str().to_string(), c.label))
        .collect();
    let existing: Vec<ExistingFeed> = feeds
        .into_iter()
        .map(|f| ExistingFeed {
            id: f.id.clone(),
            title: f.title.clone(),
            url: f.feed_url.clone(),
            website: f.website.clone(),
            category: if f.category_id == "NewsFlash.Toplevel" {
                // OPML feeds with no category outline have category ""; exposing the
                // toplevel id would turn perfect duplicates into false url-identical conflicts.
                String::new()
            } else {
                name_by_id
                    .get(&f.category_id)
                    .cloned()
                    .unwrap_or_else(|| f.category_id.clone())
            },
        })
        .collect();

    let entries = opml_import::parse_entries(&req.opml)?;
    let classification = opml_import::classify(&entries, &existing);
    let resolutions = req.resolutions.unwrap_or_default();

    if resolutions.is_empty() {
        if classification.conflicts.is_empty() {
            let (cleaned, added) =
                opml_import::build_cleaned_opml(&req.opml, &entries, &classification, &resolutions)?;
            state.engine.import_opml(&cleaned).await?;
            return Ok(Json(json!({
                "status": "imported",
                "added": added,
                "skipped": classification.exact_duplicates,
                "migrated": 0,
                "conflicts_resolved": 0,
            })));
        }
        return Ok(Json(json!({
            "status": "conflicts",
            "conflicts": classification.conflicts,
            "stats": {
                "new": classification.new_count,
                "exact_duplicates": classification.exact_duplicates,
            },
        })));
    }

    let conflict_by_key: HashMap<usize, &Conflict> = classification
        .conflicts
        .iter()
        .map(|c| (c.key, c))
        .collect();
    for resolution in &resolutions {
        let conflict = conflict_by_key
            .get(&resolution.key)
            .ok_or_else(|| ApiError::bad_request("resolution key not found"))?;
        if resolution.action == ResolutionAction::KeepExisting {
            let ok = resolution
                .keep_existing_feed_id
                .as_ref()
                .is_some_and(|id| conflict.matches.iter().any(|m| &m.id == id));
            if !ok {
                return Err(ApiError::bad_request(
                    "keep_existing_feed_id must be one of the conflict's matches",
                ));
            }
        }
    }

    let (cleaned, added) =
        opml_import::build_cleaned_opml(&req.opml, &entries, &classification, &resolutions)?;
    state.engine.import_opml(&cleaned).await?;

    let db_path = state.engine.data_dir().join("engine/data/database.sqlite");
    let mut migrated: u64 = 0;
    let mut conflicts_resolved: usize = 0;
    let mut skipped: usize = classification.exact_duplicates;
    for resolution in &resolutions {
        let conflict = conflict_by_key.get(&resolution.key).unwrap();
        conflicts_resolved += 1;
        match resolution.action {
            ResolutionAction::KeepNew => match conflict.kind {
                ConflictKind::UrlVariant => {
                    for matched in &conflict.matches {
                        if matched.id.starts_with("__file__:") {
                            continue;
                        }
                        // run the direct-SQLite migration under the engine's mutation lock
                        // (it would deadlock if held across remove_feed, which takes the same lock)
                        {
                            let _guard = state.engine.mutation_guard().await;
                            migrated += opml_import::migrate_feed_articles(
                                &db_path,
                                &matched.id,
                                &conflict.opml.url,
                            )?;
                        }
                        state.engine.remove_feed(&matched.id).await?;
                        skipped += 1;
                    }
                }
                ConflictKind::UrlIdentical => {
                    let existing_id = &conflict.matches[0].id;
                    if conflict.opml.title != conflict.matches[0].title {
                        state.engine.rename_feed(existing_id, &conflict.opml.title).await?;
                    }
                    if !conflict.opml.category.is_empty()
                        && conflict.opml.category != conflict.matches[0].category
                    {
                        let (categories, _) = state.engine.get_categories().await?;
                        let category_id = categories
                            .iter()
                            .find(|c| c.label == conflict.opml.category)
                            .map(|c| c.category_id.as_str().to_string())
                            .unwrap_or_else(|| {
                                // unreachable: cleaned opml keeps the category outline, so it exists
                                String::new()
                            });
                        if !category_id.is_empty() {
                            state.engine.move_feed(existing_id, &category_id).await?;
                        }
                    }
                }
                ConflictKind::IntraFile => {}
            },
            ResolutionAction::KeepExisting => skipped += 1,
            ResolutionAction::Skip => skipped += 1,
        }
    }

    Ok(Json(json!({
        "status": "imported",
        "added": added,
        "skipped": skipped,
        "migrated": migrated,
        "conflicts_resolved": conflicts_resolved,
    })))
}
```

- [ ] **Step 8: Run all backend tests**

Run: `cargo test`
Expected: all tests pass, including the pre-existing `import_and_export_opml` (updated), `opml_exact_duplicate_import_is_skipped`, `opml_url_variant_conflict_keeps_new_and_migrates_articles`, and all Task 1 unit tests.

- [ ] **Step 9: Format and commit**

Run: `cargo fmt`
Then:
```bash
git add src/api/sources.rs src/engine/opml_import.rs tests/sources.rs
git commit -m "feat(backend): two-phase opml import with conflict resolution and migration"
```

---

## Task 3: Frontend — PWA install/update UX + install list bullets

**Files:**
- Create: `frontend/src/pwa/usePwa.ts`
- Create: `frontend/src/components/UpdateBanner.tsx`
- Modify: `frontend/vite.config.ts` (line 11 `registerType: "autoUpdate"` → `"prompt"`)
- Modify: `frontend/src/main.tsx` (replace direct `registerSW` with side-effect import of `usePwa`)
- Modify: `frontend/src/pages/Settings.tsx` (install section rework + bullets, remove inline `beforeinstallprompt` state)
- Modify: `frontend/src/components/Shell.tsx` (render `<UpdateBanner />`)

**Interfaces:**
- Produces:
  - `usePwa(): { installPrompt: InstallEvent | null; isInstalled: boolean; needRefresh: boolean; offlineReady: boolean }`
  - `installApp(): Promise<boolean>`
  - `applyUpdate(): Promise<void>`

- [ ] **Step 1: Write `usePwa.ts`**

Create `frontend/src/pwa/usePwa.ts`:

```ts
import { useSyncExternalStore } from "react";
import { registerSW } from "virtual:pwa-register";

type InstallPrompt = Event & { prompt: () => Promise<void>; userChoice: Promise<{ outcome: "accepted" | "dismissed" }> };

interface PwaState {
  installPrompt: InstallPrompt | null;
  isInstalled: boolean;
  needRefresh: boolean;
  offlineReady: boolean;
}

let state: PwaState = { installPrompt: null, isInstalled: false, needRefresh: false, offlineReady: false };
const listeners = new Set<() => void>();

function setState(patch: Partial<PwaState>) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

function subscribe(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

function getSnapshot(): PwaState {
  return state;
}

if (typeof window !== "undefined") {
  const mql = window.matchMedia("(display-mode: standalone)");
  const isStandalone = () => mql.matches || (navigator as unknown as { standalone?: boolean }).standalone === true;
  state.isInstalled = isStandalone();
  mql.addEventListener("change", () => setState({ isInstalled: isStandalone() }));
  window.addEventListener("appinstalled", () => setState({ isInstalled: true, installPrompt: null }));
  window.addEventListener("beforeinstallprompt", (e) => {
    e.preventDefault();
    setState({ installPrompt: e as InstallPrompt });
  });

  registerSW({
    immediate: true,
    onNeedRefresh: () => setState({ needRefresh: true }),
    onOfflineReady: () => setState({ offlineReady: true }),
  });
}

export function usePwa() {
  return useSyncExternalStore(subscribe, getSnapshot);
}

export async function installApp(): Promise<boolean> {
  const prompt = state.installPrompt;
  if (!prompt) return false;
  await prompt.prompt();
  const { outcome } = await prompt.userChoice;
  if (outcome === "accepted") {
    setState({ installPrompt: null, isInstalled: true });
    return true;
  }
  return false;
}

export async function applyUpdate(): Promise<void> {
  const { updateSW } = await import("virtual:pwa-register");
  const sw = registerSW({ immediate: false, onNeedRefresh: () => setState({ needRefresh: true }) });
  // updateSW is exposed when registerType is "prompt"; call it to install the new SW and reload.
  const u = sw as unknown as { updateSW?: (reload?: boolean) => Promise<void> };
  if (u.updateSW) {
    await u.updateSW(true);
  } else {
    window.location.reload();
  }
}
```

Note: `registerSW` from `virtual:pwa-register` with `registerType: "prompt"` returns `{ updateSW }`; the exact type is generated by the plugin. If the returned object does not carry `updateSW` in your build, implement `applyUpdate` by calling `navigator.serviceWorker.getRegistration()` and `registration.waiting?.postMessage({ type: "SKIP_WAITING" })` then reloading. Verify against the generated `virtual:pwa-register` module in `frontend/node_modules` during implementation.

- [ ] **Step 2: Run typecheck — expect success with the current `autoUpdate` config**

Run (from `frontend/`): `bun run typecheck`
Expected: PASS (module compiles against current config; `onNeedRefresh`/`onOfflineReady` are valid options).

- [ ] **Step 3: Switch to `registerType: "prompt"`**

In `frontend/vite.config.ts` change line 11 to `registerType: "prompt",`.

- [ ] **Step 4: Update `main.tsx`**

Replace the direct call at line 9:
```ts
registerSW({ immediate: true });
```
with a side-effect import. At the top, replace:
```ts
import { registerSW } from "virtual:pwa-register";
```
with:
```ts
import "./pwa/usePwa";
```
(registration now happens inside the `usePwa` module).

- [ ] **Step 5: Create `UpdateBanner.tsx`**

Create `frontend/src/components/UpdateBanner.tsx`:

```tsx
import { Button } from "@heroui/react";
import { ArrowPathIcon, CheckCircleIcon } from "@heroicons/react/24/outline";
import { applyUpdate, usePwa } from "../pwa/usePwa";

export default function UpdateBanner() {
  const { needRefresh, offlineReady } = usePwa();
  if (!needRefresh && !offlineReady) return null;
  return (
    <div className="fixed bottom-4 right-4 z-50 flex items-center gap-3 rounded-lg border border-app-border-strong bg-app-surface px-4 py-3 shadow-xl">
      <span className="text-sm text-app-text-2">
        {needRefresh ? (
          <span className="flex items-center gap-2">
            <ArrowPathIcon className="h-4 w-4 shrink-0" /> A new version is available.
          </span>
        ) : (
          <span className="flex items-center gap-2">
            <CheckCircleIcon className="h-4 w-4 shrink-0 text-emerald-500" /> Ready to work offline.
          </span>
        )}
      </span>
      {needRefresh && (
        <Button size="sm" variant="primary" onPress={applyUpdate}>
          Reload
        </Button>
      )}
    </div>
  );
}
```

- [ ] **Step 6: Mount the banner in `Shell.tsx`**

In `frontend/src/components/Shell.tsx`, import and render `<UpdateBanner />` inside the returned JSX (once, near the top-level container).

- [ ] **Step 7: Rework the Settings install section + fix bullets**

In `frontend/src/pages/Settings.tsx`:

(a) Delete the local `installPrompt` state and its `useEffect` (lines 57-65) and the `InstallEvent` type (line 10).

(b) Add imports:
```ts
import { CheckIcon } from "@heroicons/react/24/outline";
import { installApp, usePwa } from "../pwa/usePwa";
```

(c) In the component, add `const { installPrompt, isInstalled } = usePwa();`

(d) Replace the whole "Install to homescreen" section (currently lines 271-297) with:

```tsx
<section className="flex flex-col gap-3 rounded-lg border border-app-border p-4">
  <p>Install to homescreen</p>
  {isInstalled ? (
    <p className="flex items-center gap-2 text-sm text-app-text-muted">
      <CheckIcon className="h-4 w-4 shrink-0 text-emerald-500" />
      App is installed.
    </p>
  ) : installPrompt ? (
    <Button onPress={installApp}>
      <ArrowLeftEndOnRectangleIcon className="h-4 w-4" />
      Install
    </Button>
  ) : (
    <>
      <p className="text-sm text-app-text-muted">App not installable as PWA.</p>
      <p className="text-xs font-medium uppercase tracking-wider text-app-text-faint">Possible reasons</p>
      <ul className="list-disc space-y-1.5 pl-5 text-sm text-app-text-muted">
        <li>App already installed</li>
        {window.location.protocol === "http:" && (
          <li>
            http: websites are not installable. If the host is private
            and known to be secure, configure{" "}
            <code>chrome://flags/#unsafely-treat-insecure-origin-as-secure</code>
          </li>
        )}
        <li>On mobile, check if the launcher supports PWA installation.</li>
      </ul>
    </>
  )}
</section>
```

- [ ] **Step 8: Typecheck, lint, build**

Run (from `frontend/`): `bun run typecheck && bun run lint && bun run build`
Expected: all pass. (The build regenerates `dist/sw.js` from the new config.)

- [ ] **Step 9: Verify list-marker audit**

Search for other marker-less content lists that should show bullets:
Run: `rg -n "<(ul|ol)[^>]*>" src --glob "*.tsx"` (from `frontend/`)
Every hit should already use `flex`/`divide` chrome styling (intentional) or a `list-disc`/`list-decimal` utility. Only the Settings install list needed a fix; if any other content list lacks a list utility, add `list-disc`/`list-decimal` + `pl-5`.

- [ ] **Step 10: Commit**

```bash
git add frontend/vite.config.ts frontend/src/main.tsx frontend/src/pwa/usePwa.ts frontend/src/components/UpdateBanner.tsx frontend/src/components/Shell.tsx frontend/src/pages/Settings.tsx
git commit -m "feat(frontend): pwa install and update ux, restore install list markers"
```

---

## Task 4: Frontend — preserve filters in article links

**Files:**
- Create: `frontend/src/utils/articleLink.ts`
- Create: `frontend/src/hooks/useArticlePath.ts`
- Modify: `frontend/src/components/TimelinePanel.tsx` (line 20)
- Modify: `frontend/src/components/ArticleListItem.tsx` (line 21)
- Modify: `frontend/src/pages/Saved.tsx` (line 43)
- Modify: `frontend/src/components/SearchBar.tsx` (line 77)

**Interfaces:**
- Produces:
  - `export function articlePath(id: string, search: URLSearchParams): string`
  - `export function useArticlePath(): (id: string) => string`

- [ ] **Step 1: Create `frontend/src/utils/articleLink.ts`**

```ts
import { encodeId } from "../api/client";

export function articlePath(id: string, search: URLSearchParams): string {
  const next = new URLSearchParams(search);
  next.delete("article");
  const query = next.toString();
  return `/feeds/${encodeId(id)}${query ? `?${query}` : ""}`;
}
```

- [ ] **Step 2: Create `frontend/src/hooks/useArticlePath.ts`**

```ts
import { useLocation } from "react-router";
import { articlePath } from "../utils/articleLink";

export function useArticlePath() {
  const location = useLocation();
  return (id: string) => articlePath(id, new URLSearchParams(location.search));
}
```

- [ ] **Step 3: Apply in `TimelinePanel.tsx`**

In `TimelineItem`, add `const articlePathFn = useArticlePath();` and change line 20:
```tsx
to={`/feeds/${encodeId(item.id)}`}
```
to:
```tsx
to={articlePathFn(item.id)}
```
Remove the now-unused `encodeId` import if it becomes unused.

- [ ] **Step 4: Apply in `ArticleListItem.tsx`**

Same change at line 21 (`to={`/feeds/${encodeId(item.id)}`}` → `to={articlePathFn(item.id)}`), adding the hook.

- [ ] **Step 5: Apply in `Saved.tsx`**

At line 43, use the hook: `to={articlePathFn(item.id)}`. (Keeps `encodeId` import only if still used elsewhere in the file — it's not, so remove it.)

- [ ] **Step 6: Apply in `SearchBar.tsx`**

At line 77, preserve the query params when navigating to a suggestion **only when already on a `/feeds*` route** (per spec; elsewhere the current page's params are unrelated to the feed list):

```ts
navigate(
  location.pathname.startsWith("/feeds")
    ? articlePath(id, new URLSearchParams(location.search))
    : `/feeds/${encodeId(id)}`,
);
```

Add `const location = useLocation();` and import `articlePath` from `../utils/articleLink`. Add `useLocation` to the existing `react-router` import at line 2.

- [ ] **Step 7: Verify**

Run (from `frontend/`): `bun run typecheck && bun run lint`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add frontend/src/utils/articleLink.ts frontend/src/hooks/useArticlePath.ts frontend/src/components/TimelinePanel.tsx frontend/src/components/ArticleListItem.tsx frontend/src/pages/Saved.tsx frontend/src/components/SearchBar.tsx
git commit -m "feat(frontend): preserve feed filters when opening an article"
```

---

## Task 5: Frontend — clickable source names + article row restructure

**Files:**
- Create: `frontend/src/components/FeedNameLink.tsx`
- Modify: `frontend/src/components/TimelinePanel.tsx` (row restructure + feed link)
- Modify: `frontend/src/components/ArticleListItem.tsx` (row restructure + feed link)
- Modify: `frontend/src/pages/Saved.tsx` (row restructure + feed link)
- Modify: `frontend/src/components/Reader.tsx` (line 77 meta feed link)

**Interfaces:**
- Consumes: `useArticlePath` from Task 4.
- Produces: `export default function FeedNameLink({ feedId, title, className }: { feedId: string; title?: string | null; className?: string })` — renders `null` when `title` is falsy, otherwise a `<Link to={/feeds?feed=<encodeId(feedId)>}>` with `onClick={(e) => e.stopPropagation()}`.

- [ ] **Step 1: Create `FeedNameLink.tsx`**

```tsx
import { Link } from "react-router";
import { encodeId } from "../api/client";

export default function FeedNameLink({
  feedId,
  title,
  className,
}: {
  feedId: string;
  title?: string | null;
  className?: string;
}) {
  if (!title) return null;
  return (
    <Link
      to={`/feeds?feed=${encodeId(feedId)}`}
      className={className}
      onClick={(e) => e.stopPropagation()}
    >
      {title}
    </Link>
  );
}
```

- [ ] **Step 2: Restructure `TimelineItem` in `TimelinePanel.tsx`**

Replace the row's single `<Link>` (lines 19-48) with a clickable row div. The final structure (keep `triggerProps` on the outer div):

```tsx
function TimelineItem({ item }: { item: Headline }) {
  const [thumbFailed, setThumbFailed] = useState(false);
  const [dialog, setDialog] = useState<"rename" | "delete" | null>(null);
  const { position, close, menuRef, triggerProps } = useContextMenu();
  const feed = useFeedSummary(item.feed_id, item.feed_title ?? "");
  const articlePathFn = useArticlePath();
  const navigate = useNavigate();

  const openArticle = () => navigate(articlePathFn(item.id));
  const openArticleNewTab = (e: React.MouseEvent) => {
    if (e.button === 1) {
      e.preventDefault();
      window.open(articlePathFn(item.id), "_blank", "noopener");
    }
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      openArticle();
    }
  };

  return (
    <>
      <div
        role="link"
        tabIndex={0}
        onClick={openArticle}
        onAuxClick={openArticleNewTab}
        onKeyDown={onKeyDown}
        className="block cursor-pointer px-4 py-2 hover:bg-app-surface/60"
        {...triggerProps}
      >
        <div className="flex items-start gap-3">
          <FeedAvatar feedId={item.feed_id} title={item.feed_title} className="h-8 w-8" />
          <div className="min-w-0 flex-1">
            {item.title && (
              <p className={`truncate text-sm ${item.unread ? "font-semibold text-app-text" : "text-app-text-2"}`}>
                {item.title}
              </p>
            )}
            <p className="truncate text-xs text-app-text-faint">
              {item.date ? formatAge(item.date) : ""}
              {item.date && item.feed_title ? " · " : ""}
              {item.feed_title && (
                <FeedNameLink
                  feedId={item.feed_id}
                  title={item.feed_title}
                  className="text-app-text-faint hover:text-accent hover:underline"
                />
              )}
            </p>
          </div>
          {item.thumbnail_url && !thumbFailed && (
            <img
              src={`/api/thumbnail/${encodeId(item.id)}`}
              alt=""
              loading="lazy"
              onError={() => setThumbFailed(true)}
              className="h-12 w-16 shrink-0 rounded-md object-cover"
            />
          )}
        </div>
      </div>
      {position && (
        <FeedContextMenu
          feed={feed}
          position={position}
          menuRef={menuRef}
          onClose={close}
          onEdit={() => setDialog("rename")}
          onDelete={() => setDialog("delete")}
        />
      )}
      <RenameDialog feed={feed} open={dialog === "rename"} onClose={() => setDialog(null)} />
      <DeleteDialog feed={feed} open={dialog === "delete"} onClose={() => setDialog(null)} />
    </>
  );
}
```

Notes: `FeedContextMenu`/`RenameDialog`/`DeleteDialog` here are placeholder until Task 6 replaces the menu; keep them for now. Imports to add: `useNavigate` from `react-router`, `FeedNameLink`, `useArticlePath`; drop `Link` if no longer used (it is still used by `TimelinePanel`'s default export? check — no, remove `Link` import if unused after the change).

- [ ] **Step 3: Restructure `ArticleListItem.tsx`**

Apply the same pattern as Step 2 (outer div `role="link"` + `onClick`/`onAuxClick`/`onKeyDown`, title stays a `<p>`, feed title becomes `FeedNameLink`). Preserve the `hideFeed` prop (render the feed name only when `!hideFeed`). Keep the existing `FeedContextMenu`/dialogs until Task 6.

- [ ] **Step 4: Restructure `Saved.tsx` rows**

`Saved.tsx` renders its own rows (lines 39-86) inline. The outer `Link` (line 43) becomes a clickable div (same pattern as `TimelineItem`), the feed title span (line 52) becomes `FeedNameLink`, and the Edit/Unsave buttons get `e.stopPropagation()`. Replace the `<li>...</li>` block (currently lines 40-85) with:

```tsx
<li key={item.id} className="py-2">
  <div
    role="link"
    tabIndex={0}
    onClick={() => navigate(articlePathFn(item.id))}
    onAuxClick={(e) => {
      if (e.button === 1) {
        e.preventDefault();
        window.open(articlePathFn(item.id), "_blank", "noopener");
      }
    }}
    onKeyDown={(e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        navigate(articlePathFn(item.id));
      }
    }}
    className="flex cursor-pointer items-start gap-2"
  >
    <div className="min-w-0 flex-1">
      {item.title && (
        <p className={`truncate text-sm ${item.unread ? "font-semibold text-app-text" : "text-app-text-2"}`}>
          {item.title}
        </p>
      )}
      <p className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-app-text-faint">
        {item.date && <span>{formatAge(item.date)}</span>}
        {item.date && item.feed_title && <span aria-hidden="true">·</span>}
        {item.feed_title && (
          <FeedNameLink
            feedId={item.feed_id}
            title={item.feed_title}
            className="truncate hover:text-accent hover:underline"
          />
        )}
      </p>
      {item.note && (
        <p className="mt-1 line-clamp-2 text-xs text-app-text-muted">{item.note}</p>
      )}
      {item.tags && item.tags.length > 0 && (
        <div className="mt-1.5 flex flex-wrap items-center gap-1">
          {item.tags.map((tag) => (
            <Chip key={tag} size="sm" variant="soft">{tag}</Chip>
          ))}
        </div>
      )}
    </div>
    <div className="flex shrink-0 items-center gap-1">
      <button
        type="button"
        aria-label="Edit note and tags"
        onClick={(e) => {
          e.stopPropagation();
          setEditId(item.id);
        }}
        className="rounded-md p-1.5 text-app-text-muted hover:bg-app-hover hover:text-app-text"
      >
        <PencilSquareIcon className="h-4 w-4" />
      </button>
      <button
        type="button"
        aria-label="Unsave"
        onClick={(e) => {
          e.stopPropagation();
          unsave.mutate({ id: item.id });
        }}
        className="rounded-md p-1.5 text-app-text-muted hover:bg-app-hover hover:text-red-500 dark:hover:text-red-400"
      >
        <XMarkIcon className="h-4 w-4" />
      </button>
    </div>
  </div>
</li>
```

Add `const navigate = useNavigate();` and `const articlePathFn = useArticlePath();` inside `Saved` (next to `unsave`). Imports to update in `Saved.tsx`: add `useNavigate` to the `react-router` import; add `useArticlePath` from `../hooks/useArticlePath`; add `FeedNameLink` from `../components/FeedNameLink`; remove `encodeId` if no longer used (it is only used in the old `Link`), and remove `Link` if unused elsewhere (it is not — remove it). The Edit/Unsave buttons keep their labels/aria.

- [ ] **Step 5: Reader meta feed link**

In `frontend/src/components/Reader.tsx`, change line 77:
```tsx
{data.feed_title && <span className="font-medium text-app-text-2">{data.feed_title}</span>}
```
to:
```tsx
{data.feed_title && (
  <FeedNameLink
    feedId={data.feed_id}
    title={data.feed_title}
    className="font-medium text-app-text-2 hover:text-accent hover:underline"
  />
)}
```
Add the `FeedNameLink` import. (Reader has no outer Link; the row div's meta is fine as a plain Link here — stopPropagation is harmless.)

- [ ] **Step 6: Verify**

Run (from `frontend/`): `bun run typecheck && bun run lint`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add frontend/src/components/FeedNameLink.tsx frontend/src/components/TimelinePanel.tsx frontend/src/components/ArticleListItem.tsx frontend/src/pages/Saved.tsx frontend/src/components/Reader.tsx
git commit -m "feat(frontend): clickable source names and article row restructure"
```

---

## Task 6: Frontend — article context menu + sidebar source menu

**Files:**
- Create: `frontend/src/components/ArticleContextMenu.tsx`
- Modify: `frontend/src/components/TimelinePanel.tsx` (swap `FeedContextMenu` → `ArticleContextMenu`)
- Modify: `frontend/src/components/ArticleListItem.tsx` (same swap)
- Modify: `frontend/src/components/Sidebar.tsx` (add right-click source menu to `FeedLink`)

**Interfaces:**
- Consumes: `ContextMenuPosition` from `../hooks/useContextMenu`; `Headline`; `FeedSummary` (via `useFeedSummary`); `useMarkRead`, `useSaveArticle`, `useUnsaveArticle`, `useFeedRead` from `../state/hooks`; `useNavigate`/`useArticlePath` from Task 4.
- Produces: `export default function ArticleContextMenu({ article, position, menuRef, onClose }: { article: Headline; position: ContextMenuPosition; menuRef: React.RefObject<HTMLDivElement | null>; onClose: () => void })`

- [ ] **Step 1: Create `ArticleContextMenu.tsx`**

```tsx
import { useLayoutEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import {
  ArrowTopRightOnSquareIcon,
  BookmarkIcon,
  CheckIcon,
  ClipboardDocumentIcon,
  EyeIcon,
  EyeSlashIcon,
  FolderIcon,
  ShareIcon,
  XMarkIcon,
} from "@heroicons/react/24/outline";
import type { Headline } from "../api/types";
import { encodeId } from "../api/client";
import { useArticlePath } from "../hooks/useArticlePath";
import { useFeedRead, useMarkRead, useSaveArticle, useUnsaveArticle } from "../state/hooks";
import type { ContextMenuPosition } from "../hooks/useContextMenu";

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

async function shareArticle(title: string | null, url: string | null): Promise<"shared" | "copied"> {
  if (!url) return "copied";
  if (navigator.share) {
    try {
      await navigator.share({ title: title ?? "", url });
      return "shared";
    } catch {
      // dismissed — do nothing
    }
  }
  return (await copyText(url)) ? "copied" : "copied";
}

export default function ArticleContextMenu({
  article,
  position,
  menuRef,
  onClose,
}: {
  article: Headline;
  position: ContextMenuPosition;
  menuRef: React.RefObject<HTMLDivElement | null>;
  onClose: () => void;
}) {
  const itemRef = useRef<HTMLDivElement>(null);
  const [clamped, setClamped] = useState(position);
  const [copied, setCopied] = useState<string | null>(null);
  const markRead = useMarkRead();
  const save = useSaveArticle();
  const unsave = useUnsaveArticle();
  const feedRead = useFeedRead();
  const navigate = useNavigate();
  const articlePathFn = useArticlePath();

  useLayoutEffect(() => {
    const el = itemRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const maxX = Math.max(4, window.innerWidth - rect.width - 4);
    const maxY = Math.max(4, window.innerHeight - rect.height - 4);
    setClamped({ x: Math.min(Math.max(position.x, 4), maxX), y: Math.min(Math.max(position.y, 4), maxY) });
  }, [position]);

  const itemClass =
    "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm text-app-text-2 hover:bg-app-hover hover:text-app-text disabled:cursor-not-allowed disabled:opacity-50";

  const flash = (label: string) => {
    setCopied(label);
    onClose();
  };

  const actions = [
    {
      label: "Open in browser",
      icon: ArrowTopRightOnSquareIcon,
      disabled: !article.url,
      onPress: () => {
        if (article.url) window.open(article.url, "_blank", "noopener");
        onClose();
      },
    },
    {
      label: "Copy URL",
      icon: ClipboardDocumentIcon,
      disabled: !article.url,
      onPress: async () => flash((await copyText(article.url ?? "")) ? "URL copied" : ""),
    },
    {
      label: "Copy title",
      icon: ClipboardDocumentIcon,
      disabled: !article.title,
      onPress: async () => flash((await copyText(article.title ?? "")) ? "Title copied" : ""),
    },
    {
      label: "Share",
      icon: ShareIcon,
      disabled: !article.url,
      onPress: async () => flash((await shareArticle(article.title, article.url)) === "copied" ? "URL copied" : ""),
    },
    { separator: true },
    {
      label: article.unread ? "Mark read" : "Mark unread",
      icon: article.unread ? EyeIcon : EyeSlashIcon,
      onPress: () => {
        markRead.mutate({ id: article.id, read: article.unread });
        onClose();
      },
    },
    {
      label: article.marked ? "Unsave" : "Save",
      icon: article.marked ? XMarkIcon : BookmarkIcon,
      onPress: () => {
        if (article.marked) {
          unsave.mutate({ id: article.id });
        } else {
          save.mutate({ id: article.id, note: undefined, tags: [] });
        }
        onClose();
      },
    },
    { separator: true },
    {
      label: "Open feed",
      icon: FolderIcon,
      onPress: () => {
        navigate(`/feeds?feed=${encodeId(article.feed_id)}`);
        onClose();
      },
    },
    {
      label: "Mark all read",
      icon: CheckIcon,
      onPress: () => {
        feedRead.mutate({ id: article.feed_id });
        onClose();
      },
    },
  ] as const;

  return (
    <div
      ref={(node) => {
        menuRef.current = node;
        itemRef.current = node;
      }}
      data-context-menu
      role="menu"
      className="fixed z-50 min-w-48 rounded-md border border-app-border-strong bg-app-bg py-1 shadow-xl"
      style={{ left: clamped.x, top: clamped.y }}
    >
      {actions.map((action, index) =>
        "separator" in action ? (
          <div key={index} role="separator" className="my-1 border-t border-app-border/60" />
        ) : (
          <button
            key={index}
            type="button"
            role="menuitem"
            className={itemClass}
            disabled={"disabled" in action && action.disabled}
            onClick={action.onPress}
          >
            <action.icon className="h-4 w-4 shrink-0" />
            {action.label}
          </button>
        ),
      )}
    </div>
  );
}
```

Note: the `flash`/`copied` state is intentionally minimal — the menu closes immediately after an action; if you prefer showing "Copied", keep the `copied` state for a small delay instead of calling `onClose()` right away. The behavior must remain: every action closes the menu.

- [ ] **Step 2: Wire into `TimelinePanel.tsx`**

Replace the `FeedContextMenu` block (currently lines 49-58) with:

```tsx
{position && (
  <ArticleContextMenu article={item} position={position} menuRef={menuRef} onClose={close} />
)}
```
Remove the now-unused `FeedContextMenu` import and `RenameDialog`/`DeleteDialog` + `dialog` state from `TimelineItem` (Task 5 left them as placeholders). `useFeedSummary` is no longer needed in `TimelinePanel`.

- [ ] **Step 3: Wire into `ArticleListItem.tsx`**

Same swap as Step 2. Remove `FeedContextMenu`, `RenameDialog`, `DeleteDialog`, `useFeedSummary` usage, and the `dialog` state.

- [ ] **Step 4: Add the source menu to sidebar rows**

In `frontend/src/components/Sidebar.tsx`:

(a) Imports: add `useContextMenu` from `../hooks/useContextMenu`, `FeedContextMenu` from `./FeedContextMenu`, `RenameDialog`/`DeleteDialog` from `./SourceMenu`, and `useState`.

(b) Update `FeedLink` to accept a full feed summary and attach the menu:

```tsx
function FeedLink({ feed }: { feed: FeedSummary }) {
  const { position, close, menuRef, triggerProps } = useContextMenu();
  const [dialog, setDialog] = useState<"rename" | "delete" | null>(null);
  return (
    <>
      <Link
        to={`/feeds?feed=${encodeURIComponent(feed.id)}`}
        className="flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-sm transition-colors text-app-text-muted hover:bg-app-hover/60 hover:text-app-text"
        {...triggerProps}
      >
        <span className="truncate">{feed.title}</span>
        {feed.unread_count > 0 && (
          <span className="rounded bg-app-surface-2 px-1.5 py-0.5 text-[10px] font-semibold text-app-text-2">
            {feed.unread_count}
          </span>
        )}
      </Link>
      {position && (
        <FeedContextMenu
          feed={feed}
          position={position}
          menuRef={menuRef}
          onClose={close}
          onEdit={() => setDialog("rename")}
          onDelete={() => setDialog("delete")}
        />
      )}
      <RenameDialog feed={feed} open={dialog === "rename"} onClose={() => setDialog(null)} />
      <DeleteDialog feed={feed} open={dialog === "delete"} onClose={() => setDialog(null)} />
    </>
  );
}
```

(c) Update the caller (line ~210) from `<FeedLink key={feed.id} id={feed.id} title={feed.title} unreadCount={feed.unread_count} />` to `<FeedLink key={feed.id} feed={feed} />`. Import `FeedSummary` from `../api/types`.

Note: The `Link` for sidebar rows must remain a `Link` (it stays a NavLink-style row; the context-menu handlers attach fine). If `className` computation currently uses `NavLink`'s `isActive`, Task 7 replaces that logic — for this task, keep the existing class computation exactly as-is.

- [ ] **Step 5: Verify**

Run (from `frontend/`): `bun run typecheck && bun run lint`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/ArticleContextMenu.tsx frontend/src/components/TimelinePanel.tsx frontend/src/components/ArticleListItem.tsx frontend/src/components/Sidebar.tsx
git commit -m "feat(frontend): article context menu and sidebar source menu"
```

---

## Task 7: Frontend — filter chips + sidebar active state

**Files:**
- Modify: `frontend/src/components/Sidebar.tsx` (`TreeLink`, `FeedLink`, "All" row active logic)
- Modify: `frontend/src/pages/Feeds.tsx` (filter chip bar)

**Interfaces:**
- Consumes: `useSources`, `useCategories` from `../state/hooks`; `CategoryNode` from `../api/types`.
- Produces: none (self-contained UI).

- [ ] **Step 1: Sidebar active-state logic**

In `Sidebar.tsx`:

(a) Add a hook near the top (below the imports). Add `useSearchParams` to the existing `react-router` import at line 3 (`import { Link, NavLink, useLocation, useSearchParams } from "react-router"`):

```tsx
function useFeedsFilter(): { feed: string | null; category: string | null } {
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const onFeeds = location.pathname === "/feeds" || location.pathname.startsWith("/feeds/");
  if (!onFeeds) return { feed: null, category: null };
  return { feed: searchParams.get("feed"), category: searchParams.get("category") };
}
```

(b) Rewrite `TreeLink` to compute active state:

```tsx
function TreeLink({ node, depth }: { node: CategoryNode; depth: number }) {
  const { feed, category } = useFeedsFilter();
  const active = !feed && category === node.category_id;
  return (
    <li>
      <Link
        to={`/feeds?category=${encodeURIComponent(node.category_id)}`}
        className={clsx(
          "flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
          depth > 0 && "ml-3",
          active
            ? "border-l-2 border-accent bg-accent-soft text-accent-soft-foreground"
            : "text-app-text-muted hover:bg-app-hover/60 hover:text-app-text",
        )}
      >
        <span className="truncate">{node.name}</span>
        {node.unread_count > 0 && (
          <span className="rounded bg-app-surface-2 px-1.5 py-0.5 text-[10px] font-semibold text-app-text-2">
            {node.unread_count}
          </span>
        )}
      </Link>
      {node.children.length > 0 && (
        <ul className="mt-0.5 flex flex-col gap-0.5">
          {node.children.map((child) => (
            <TreeLink key={child.category_id} node={child} depth={depth + 1} />
          ))}
        </ul>
      )}
    </li>
  );
}
```

(c) Rewrite `FeedLink` active state similarly (replace the `NavLink` with `Link`, `active = feed === feed.id`, same `border-l-2 border-accent` active style).

(d) Update the "All" row (currently `Sidebar.tsx:180-185`) to be active when no feed/category filter:

```tsx
const { feed, category } = useFeedsFilter();
...
<Link
  to="/feeds"
  className={clsx(
    "flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
    !feed && !category
      ? "border-l-2 border-accent bg-accent-soft text-accent-soft-foreground"
      : "text-app-text-muted hover:bg-app-hover/60 hover:text-app-text",
  )}
>
  <span>All</span>
</Link>
```

(e) Remove the now-unused `NavLink` import if nothing else uses it (the top nav `NavItem` still uses `NavLink` — keep the import).

- [ ] **Step 2: Filter chips in `Feeds.tsx`**

Add a `FilterChips` component (can live at the bottom of `Feeds.tsx`):

```tsx
function useFilterChips() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { data: sourcesData } = useSources();
  const { data: categoriesData } = useCategories();

  const feedId = searchParams.get("feed");
  const categoryId = searchParams.get("category");
  const saved = searchParams.get("saved");
  const unread = searchParams.get("unread");
  const tag = searchParams.get("tag");
  const q = searchParams.get("q") ?? "";

  const feedName = useMemo(() => {
    const all = sourcesData?.groups.flatMap((g) => g.feeds) ?? [];
    return all.find((f) => f.id === feedId)?.title ?? feedId ?? "";
  }, [sourcesData, feedId]);

  const categoryName = useMemo(() => {
    const walk = (nodes: CategoryNode[]): string => {
      for (const node of nodes) {
        if (node.category_id === categoryId) return node.name;
        const found = walk(node.children);
        if (found) return found;
      }
      return "";
    };
    return categoriesData ? walk(categoriesData.categories) : "";
  }, [categoriesData, categoryId]);

  const remove = (key: string) => {
    const next = new URLSearchParams(searchParams);
    next.delete(key);
    setSearchParams(next);
  };
  const clearAll = () => {
    const next = new URLSearchParams(searchParams);
    for (const key of ["feed", "category", "saved", "unread", "tag", "q"]) {
      next.delete(key);
    }
    setSearchParams(next);
  };

  const chips: { key: string; label: string; remove: () => void }[] = [];
  if (feedId) chips.push({ key: "feed", label: feedName, remove: () => remove("feed") });
  if (categoryId) chips.push({ key: "category", label: categoryName || "Category", remove: () => remove("category") });
  if (q.trim()) chips.push({ key: "q", label: `“${q.trim()}”`, remove: () => remove("q") });
  if (saved) chips.push({ key: "saved", label: "Saved", remove: () => remove("saved") });
  if (unread) chips.push({ key: "unread", label: "Unread", remove: () => remove("unread") });
  if (tag) chips.push({ key: "tag", label: `#${tag}`, remove: () => remove("tag") });

  return { chips, clearAll };
}
```

And render it in `Feeds` between the header and the `SearchBar` (insert after line 55):

```tsx
const { chips, clearAll } = useFilterChips();
...
{chips.length > 0 && (
  <div className="flex flex-wrap items-center gap-1.5 px-4 pt-3">
    {chips.map((chip) => (
      <button
        key={chip.key}
        type="button"
        onClick={chip.remove}
        title="Remove filter"
        className="group inline-flex items-center gap-1 rounded-full border border-accent/40 bg-accent-soft px-2.5 py-0.5 text-xs font-medium text-accent-soft-foreground hover:bg-accent-soft-hover"
      >
        {chip.label}
        <XMarkIcon className="h-3 w-3" />
      </button>
    ))}
    {chips.length > 1 && (
      <button
        type="button"
        onClick={clearAll}
        className="text-xs font-medium text-app-text-muted hover:text-app-text"
      >
        Clear all
      </button>
    )}
  </div>
)}
```

Imports to add in `Feeds.tsx`: `XMarkIcon` from `@heroicons/react/24/outline`, `useMemo` from `react`, `useSources`, `useCategories` from `../state/hooks`, `CategoryNode` from `../api/types`, `useSearchParams` is already imported.

- [ ] **Step 3: Verify**

Run (from `frontend/`): `bun run typecheck && bun run lint`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/Sidebar.tsx frontend/src/pages/Feeds.tsx
git commit -m "feat(frontend): active filter chips and sidebar filter highlighting"
```

---

## Task 8: Frontend — OPML conflict resolution dialog

**Files:**
- Create: `frontend/src/components/OpmlConflictDialog.tsx`
- Modify: `frontend/src/api/types.ts` (add OPML response/conflict types)
- Modify: `frontend/src/state/hooks.ts` (`useImportOpml` response handling)
- Modify: `frontend/src/components/OpmlButtons.tsx` and `frontend/src/components/OpmlImportButton.tsx` (dialog wiring + result summary)

**Interfaces:**
- Consumes: the JSON contract from Global Constraints (Task 2 backend shape).
- Produces:
  - TS types (in `frontend/src/api/types.ts`):
    ```ts
    export interface OpmlEntry { index: number; title: string; url: string; category: string }
    export interface OpmlExistingFeed { id: string; title: string; url: string | null; website: string | null; category: string }
    export interface OpmlConflict { key: number; kind: "url-identical" | "url-variant" | "intra-file"; opml: OpmlEntry; matches: OpmlExistingFeed[] }
    export interface OpmlResolution { key: number; action: "keep-new" | "keep-existing" | "skip"; keep_existing_feed_id?: string }
    export interface ImportOpmlResponse {
      status: "imported" | "conflicts";
      added?: number;
      skipped?: number;
      migrated?: number;
      conflicts_resolved?: number;
      conflicts?: OpmlConflict[];
      stats?: { new: number; exact_duplicates: number };
    }
    ```
  - `useImportOpml()` returns the mutation; its `mutateAsync` resolves to `ImportOpmlResponse`.

- [ ] **Step 1: Add TS types**

Append the `ImportOpmlResponse`-related interfaces above to `frontend/src/api/types.ts`.

- [ ] **Step 2: Update `useImportOpml`**

In `frontend/src/state/hooks.ts`, change `useImportOpml` (lines 333-345) to:

```ts
export function useImportOpml() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ opml, resolutions }: { opml: string; resolutions?: OpmlResolution[] }) =>
      api.post<ImportOpmlResponse>("/api/sources/import-opml", { opml, resolutions }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["feeds"] });
      queryClient.invalidateQueries({ queryKey: ["sources"] });
      queryClient.invalidateQueries({ queryKey: ["overview"] });
      queryClient.invalidateQueries({ queryKey: ["categories"] });
      queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });
}
```
Add `OpmlResolution`, `ImportOpmlResponse` to the type imports.

- [ ] **Step 3: Create `OpmlConflictDialog.tsx`**

```tsx
import { useMemo, useState } from "react";
import { Button, Modal, useOverlayState } from "@heroui/react";
import { useImportOpml } from "../state/hooks";
import type { ImportOpmlResponse, OpmlConflict, OpmlResolution } from "../api/types";

export default function OpmlConflictDialog({
  open,
  opml,
  conflicts,
  onClose,
  onImported,
}: {
  open: boolean;
  opml: string;
  conflicts: OpmlConflict[];
  onClose: () => void;
  onImported: (result: ImportOpmlResponse) => void;
}) {
  const state = useOverlayState({ isOpen: open, onOpenChange: (isOpen) => { if (!isOpen) onClose(); } });
  const importOpml = useImportOpml();
  const [choices, setChoices] = useState<Record<number, OpmlResolution>>({});
  const [error, setError] = useState("");

  const defaultResolution = (conflict: OpmlConflict): OpmlResolution => {
    const existing = conflict.matches.find((m) => !m.id.startsWith("__file__:")) ?? conflict.matches[0];
    if (existing) {
      return { key: conflict.key, action: "keep-existing", keep_existing_feed_id: existing.id };
    }
    return { key: conflict.key, action: "keep-new" };
  };

  const initialChoices = useMemo(
    () => Object.fromEntries(conflicts.map((c) => [c.key, defaultResolution(c)])),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [open, conflicts],
  );

  const choiceFor = (conflict: OpmlConflict): OpmlResolution =>
    choices[conflict.key] ?? initialChoices[conflict.key] ?? defaultResolution(conflict);

  const submit = async () => {
    setError("");
    const resolutions = conflicts.map((c) => choiceFor(c));
    try {
      const result = await importOpml.mutateAsync({ opml, resolutions });
      onImported(result);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Import failed");
    }
  };

  const kindLabel: Record<OpmlConflict["kind"], string> = {
    "url-identical": "Same feed URL, different details",
    "url-variant": "Same feed, different URL",
    "intra-file": "Duplicate within the file",
  };

  return (
    <Modal state={state}>
      <Modal.Backdrop>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>Resolve duplicate feeds</Modal.Heading>
            </Modal.Header>
            <Modal.Body>
              <p className="text-sm text-app-text-muted">
                The imported file has {conflicts.length} feed{conflicts.length > 1 ? "s" : ""} that match existing sources. Choose which to keep.
              </p>
              <div className="mt-4 flex flex-col gap-4">
                {conflicts.map((conflict) => {
                  const choice = choiceFor(conflict);
                  return (
                    <div key={conflict.key} className="rounded-lg border border-app-border p-3">
                      <p className="text-xs font-medium uppercase tracking-wider text-app-text-faint">{kindLabel[conflict.kind]}</p>
                      <div className="mt-2 flex items-start gap-3">
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-sm font-semibold text-app-text">{conflict.opml.title}</p>
                          <p className="truncate text-xs text-app-text-muted">{conflict.opml.url}</p>
                          <p className="truncate text-xs text-app-text-faint">{conflict.opml.category || "Uncategorized"}</p>
                        </div>
                        <div className="flex shrink-0 flex-col gap-1">
                          {conflict.matches.filter((m) => !m.id.startsWith("__file__:")).map((match) => (
                            <label key={match.id} className="flex items-center gap-2 text-sm text-app-text-2">
                              <input
                                type="radio"
                                name={`conflict-${conflict.key}`}
                                checked={choice.action === "keep-existing" && choice.keep_existing_feed_id === match.id}
                                onChange={() =>
                                  setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-existing", keep_existing_feed_id: match.id } }))
                                }
                              />
                              <span className="min-w-0">
                                <span className="block truncate">{match.title}</span>
                                <span className="block truncate text-xs text-app-text-faint">{match.url ?? ""}</span>
                              </span>
                            </label>
                          ))}
                          <label className="flex items-center gap-2 text-sm text-app-text-2">
                            <input
                              type="radio"
                              name={`conflict-${conflict.key}`}
                              checked={choice.action === "keep-new"}
                              onChange={() =>
                                setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "keep-new" } }))
                              }
                            />
                            <span>
                              <span className="block">Keep new</span>
                              <span className="block truncate text-xs text-app-text-faint">{conflict.opml.url}</span>
                            </span>
                          </label>
                          <label className="flex items-center gap-2 text-sm text-app-text-2">
                            <input
                              type="radio"
                              name={`conflict-${conflict.key}`}
                              checked={choice.action === "skip"}
                              onChange={() =>
                                setChoices((prev) => ({ ...prev, [conflict.key]: { key: conflict.key, action: "skip" } }))
                              }
                            />
                            <span>Skip</span>
                          </label>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
              {error && <p className="mt-3 text-sm text-red-600 dark:text-red-400">{error}</p>}
            </Modal.Body>
            <Modal.Footer>
              <Button variant="ghost" size="sm" onPress={onClose} isDisabled={importOpml.isPending}>
                Cancel
              </Button>
              <Button variant="primary" size="sm" onPress={submit} isDisabled={importOpml.isPending}>
                {importOpml.isPending ? "Importing…" : "Apply"}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
```

- [ ] **Step 4: Wire into `OpmlButtons.tsx` and `OpmlImportButton.tsx`**

Both components currently call `importOpml.mutateAsync({ opml })` and expect immediate success. Both now: read the file → `mutateAsync({ opml })` → if `result.status === "conflicts"`, open the dialog; else show a summary. They share the dialog and the summary text. Replace the whole file contents:

`frontend/src/components/OpmlButtons.tsx`:
```tsx
import { useRef, useState } from "react";
import { Button } from "@heroui/react";
import { ArrowDownTrayIcon, ArrowUpTrayIcon } from "@heroicons/react/24/outline";
import { useExportOpml, useImportOpml } from "../state/hooks";
import { formatError } from "./Feedback";
import type { ImportOpmlResponse, OpmlConflict } from "../api/types";
import OpmlConflictDialog from "./OpmlConflictDialog";

function summaryText(result: ImportOpmlResponse): string {
  const migrated = (result.migrated ?? 0) > 0 ? `, migrated ${result.migrated}` : "";
  return `Imported ${result.added} feed(s), skipped ${result.skipped} duplicate(s)${migrated}`;
}

export default function OpmlButtons() {
  const importOpml = useImportOpml();
  const exportOpml = useExportOpml();
  const fileRef = useRef<HTMLInputElement>(null);
  const [status, setStatus] = useState("");
  const [conflictState, setConflictState] = useState<{ opml: string; conflicts: OpmlConflict[] } | null>(null);

  const onFile = async (file: File | null) => {
    if (!file) return;
    setStatus("");
    try {
      const text = await file.text();
      const result = await importOpml.mutateAsync({ opml: text });
      if (result.status === "conflicts") {
        setConflictState({ opml: text, conflicts: result.conflicts ?? [] });
      } else {
        setStatus(summaryText(result));
      }
    } catch (e) {
      setStatus(formatError(e));
    }
  };

  const onExport = async () => {
    setStatus("");
    try {
      const xml = await exportOpml.mutateAsync();
      const blob = new Blob([xml], { type: "text/xml" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = "feedea-subscriptions.opml";
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      setStatus("Exported");
    } catch (e) {
      setStatus(formatError(e));
    }
  };

  return (
    <div className="relative flex flex-wrap items-center gap-2">
      <input
        ref={fileRef}
        type="file"
        accept=".opml,.xml,text/xml,application/xml"
        className="hidden"
        onChange={(e) => {
          void onFile(e.target.files?.[0] ?? null);
          e.target.value = "";
        }}
      />
      <Button size="sm" variant="secondary" onPress={() => fileRef.current?.click()} isDisabled={importOpml.isPending}>
        <ArrowUpTrayIcon className="h-4 w-4" />
        Import
      </Button>
      <Button size="sm" variant="secondary" onPress={onExport} isDisabled={exportOpml.isPending}>
        <ArrowDownTrayIcon className="h-4 w-4" />
        Export
      </Button>
      {status && (
        <span className="absolute right-0 top-full z-10 mt-1 max-w-48 truncate rounded-md border border-app-border bg-app-bg px-2 py-1 text-xs text-app-text-faint shadow-lg">
          {status}
        </span>
      )}
      {conflictState && (
        <OpmlConflictDialog
          open
          opml={conflictState.opml}
          conflicts={conflictState.conflicts}
          onClose={() => setConflictState(null)}
          onImported={(result) => setStatus(summaryText(result))}
        />
      )}
    </div>
  );
}
```

`frontend/src/components/OpmlImportButton.tsx`:
```tsx
import { useRef, useState } from "react";
import { ArrowUpTrayIcon } from "@heroicons/react/24/outline";
import { useImportOpml } from "../state/hooks";
import { formatError } from "./Feedback";
import type { ImportOpmlResponse, OpmlConflict } from "../api/types";
import OpmlConflictDialog from "./OpmlConflictDialog";

function summaryText(result: ImportOpmlResponse): string {
  const migrated = (result.migrated ?? 0) > 0 ? `, migrated ${result.migrated}` : "";
  return `Imported ${result.added} feed(s), skipped ${result.skipped} duplicate(s)${migrated}`;
}

export default function OpmlImportButton({ className }: { className?: string }) {
  const importOpml = useImportOpml();
  const fileRef = useRef<HTMLInputElement>(null);
  const [status, setStatus] = useState("");
  const [conflictState, setConflictState] = useState<{ opml: string; conflicts: OpmlConflict[] } | null>(null);

  const onFile = async (file: File | null) => {
    if (!file) return;
    setStatus("");
    try {
      const text = await file.text();
      const result = await importOpml.mutateAsync({ opml: text });
      if (result.status === "conflicts") {
        setConflictState({ opml: text, conflicts: result.conflicts ?? [] });
      } else {
        setStatus(summaryText(result));
      }
    } catch (e) {
      setStatus(formatError(e));
    }
  };

  return (
    <span className={`relative ${className ?? ""}`}>
      <input
        ref={fileRef}
        type="file"
        accept=".opml,.xml,text/xml,application/xml"
        className="hidden"
        onChange={(e) => {
          void onFile(e.target.files?.[0] ?? null);
          e.target.value = "";
        }}
      />
      <button
        type="button"
        aria-label="Import OPML"
        title="Import OPML"
        onClick={() => fileRef.current?.click()}
        disabled={importOpml.isPending}
        className="flex items-center justify-center rounded-md p-1.5 text-app-text-muted transition-colors hover:bg-app-hover/60 hover:text-app-text disabled:cursor-not-allowed disabled:opacity-50"
      >
        <ArrowUpTrayIcon className="h-4 w-4" />
      </button>
      {status && (
        <span className="absolute right-0 top-full z-10 mt-1 max-w-48 truncate rounded-md border border-app-border bg-app-bg px-2 py-1 text-xs text-app-text-faint shadow-lg">
          {status}
        </span>
      )}
      {conflictState && (
        <OpmlConflictDialog
          open
          opml={conflictState.opml}
          conflicts={conflictState.conflicts}
          onClose={() => setConflictState(null)}
          onImported={(result) => setStatus(summaryText(result))}
        />
      )}
    </span>
  );
}
```

- [ ] **Step 5: Verify**

Run (from `frontend/`): `bun run typecheck && bun run lint`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/api/types.ts frontend/src/state/hooks.ts frontend/src/components/OpmlConflictDialog.tsx frontend/src/components/OpmlButtons.tsx frontend/src/components/OpmlImportButton.tsx
git commit -m "feat(frontend): opml conflict resolution dialog"
```

---

## Task 9: Full verification and smoke test

**Files:** none (verification only).

- [ ] **Step 1: Full backend + frontend verification**

Run from repo root: `make test`
Expected: frontend builds, `cargo test` passes (all unit + integration tests), `bun run typecheck` and `bun run lint` pass.

- [ ] **Step 2: Manual smoke test**

Start the app (`make run` or `cargo run -- --data-dir /tmp/feedea-smoke --port 3000` plus the built frontend) and verify at least:
- Feeds list: right-click an article → article menu shows Open/Copy/Share/read/save/Open feed; the row still navigates on left click; the feed name link navigates to `/feeds?feed=…` and keeps the filter visible in the sidebar.
- Open an article from a category-filtered list → the URL keeps `?category=…`, sidebar highlights the category, and the filter chip is visible; back button returns to the filtered list.
- Settings → Install section shows the correct state (Installed / Install button / not-installable reasons with bullets).
- Sources page → import an OPML containing a feed that already exists (same URL, different title) → conflict dialog appears; picking "keep new" migrates articles (verify via articles list).

- [ ] **Step 3: Commit any fixes**

If any of the above surfaced bugs, fix them in small commits; otherwise no commit needed.

---

## Execution order (conflict-free parallelism)

Sequential in this order due to shared frontend files:
1. **Batch A (parallel, disjoint files):** Task 1 (backend) · Task 3 (PWA) · Task 4 (links) · Task 8 (conflict dialog).
2. Task 2 (backend endpoint) — depends on Task 1.
3. Task 5 (clickable feeds) — depends on Task 4 (uses `useArticlePath`, rewrites the same rows).
4. Task 6 (context menus) — depends on Task 5 (same rows + sidebar).
5. Task 7 (filter chips + sidebar) — depends on Task 6 (sidebar).
6. Task 9 (final verification) — after everything.

If a later task is blocked only by review latency of an earlier one, you may run independent batches concurrently; never edit the same file in two parallel tasks. Tasks 4, 5, 6, 7 all touch `Sidebar.tsx` or `TimelinePanel.tsx`/`ArticleListItem.tsx` and must run sequentially in the order given.
