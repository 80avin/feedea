use std::collections::{HashMap, HashSet};

use opml::{OPML, Outline};
use serde::{Deserialize, Serialize};

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
            let child_category = if title.is_empty() {
                category.to_string()
            } else {
                title
            };
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

fn outline_title(o: &Outline) -> String {
    o.title
        .clone()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| o.text.clone())
}

fn merge_sibling_categories(outlines: &mut Vec<Outline>) {
    struct Holder {
        is_category: bool,
        title: String,
        outline: Outline,
    }
    let mut groups: Vec<Holder> = Vec::with_capacity(outlines.len());
    for mut outline in std::mem::take(outlines) {
        let is_category = outline.xml_url.is_none();
        let title = outline_title(&outline);
        if is_category
            && let Some(existing) = groups
                .iter_mut()
                .find(|g| g.is_category && g.title == title)
        {
            existing.outline.outlines.append(&mut outline.outlines);
            continue;
        }
        groups.push(Holder {
            is_category,
            title,
            outline,
        });
    }
    // Recurse AFTER all appends so a merged parent's children are
    // normalized as one level (e.g. "Sub" siblings that each came from a
    // different duplicate "Top" also merge).
    *outlines = groups
        .into_iter()
        .map(|mut holder| {
            merge_sibling_categories(&mut holder.outline.outlines);
            holder.outline
        })
        .collect();
}

pub fn classify(entries: &[OpmlEntry], existing: &[ExistingFeed]) -> Classification {
    let mut conflicts = Vec::new();
    let mut skipped = HashSet::new();
    let mut new_count = 0;
    let mut first_by_url: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        let norm = normalize_url(&entry.url);
        let is_first = norm
            .as_ref()
            .map(|n| !first_by_url.contains_key(n))
            .unwrap_or(true);

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

pub fn migrate_feed_articles(
    db_path: &std::path::Path,
    from_feed_id: &str,
    to_feed_id: &str,
) -> anyhow::Result<u64> {
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
    // keep index assignment in the same depth-first order as parse_entries/collect
    let mut result = Vec::with_capacity(outlines.len());
    for outline in outlines.iter_mut() {
        if outline.xml_url.is_some() {
            let idx = *index;
            *index += 1;
            if keep.contains(&idx) {
                result.push(outline.clone());
            }
        } else {
            filter_outlines(&mut outline.outlines, index, keep);
            result.push(outline.clone());
        }
    }
    *outlines = result;
}

pub fn build_cleaned_opml(
    opml_str: &str,
    entries: &[OpmlEntry],
    classification: &Classification,
    resolutions: &[Resolution],
) -> anyhow::Result<(String, usize)> {
    let mut doc = OPML::from_str(opml_str).map_err(|e| anyhow::anyhow!("invalid opml: {e}"))?;
    let keep_new = keep_new_keys(resolutions);
    let conflict_by_key: HashMap<usize, &Conflict> = classification
        .conflicts
        .iter()
        .map(|c| (c.key, c))
        .collect();

    // For intra-file conflicts resolved to keep-new, the later occurrence wins that url.
    // Key by the normalized url so every variant of that url resolves to the winner.
    // Iterate keys ascending so the LAST (later/higher-index) keep-new occurrence wins
    // deterministically (HashMap iteration order was nondeterministic).
    let mut intra_winner: HashMap<String, usize> = HashMap::new();
    let mut keys: Vec<&usize> = conflict_by_key.keys().collect();
    keys.sort();
    for key in keys {
        let conflict = conflict_by_key[key];
        if conflict.kind == ConflictKind::IntraFile
            && keep_new.contains(key)
            && let Some(n) = normalize_url(&conflict.opml.url)
        {
            intra_winner.insert(n, conflict.opml.index);
        }
    }

    let mut keep = HashSet::new();
    for entry in entries {
        if classification.skipped.contains(&entry.index) {
            continue;
        }
        let winner = normalize_url(&entry.url)
            .as_ref()
            .and_then(|n| intra_winner.get(n))
            .copied();
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
                    if winner == Some(entry.index) {
                        keep.insert(entry.index);
                    }
                }
            }
        } else if winner.is_none_or(|w| w == entry.index) {
            // brand-new feed
            keep.insert(entry.index);
        }
    }

    let mut index = 0usize;
    filter_outlines(&mut doc.body.outlines, &mut index, &keep);
    merge_sibling_categories(&mut doc.body.outlines);
    let cleaned = doc
        .to_string()
        .map_err(|e| anyhow::anyhow!("serialize opml: {e}"))?;
    Ok((cleaned, keep.len()))
}

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
        assert_eq!(
            normalize_url("https://example.com"),
            Some("https://example.com".into())
        );
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

    #[test]
    fn build_cleaned_opml_drops_exact_dups_keeps_resolved() {
        use crate::engine::opml_import::*;
        let opml = OPML.to_string();
        let entries = parse_entries(&opml).unwrap();
        let classification = classify(&entries, &existing());
        // entry 0 is an exact duplicate (skipped). Entries 1 and 2 are url-variant conflicts.
        let resolutions = vec![
            Resolution {
                key: 1,
                action: ResolutionAction::KeepExisting,
                keep_existing_feed_id: None,
            },
            Resolution {
                key: 2,
                action: ResolutionAction::KeepNew,
                keep_existing_feed_id: None,
            },
        ];
        let (cleaned, added) =
            build_cleaned_opml(&opml, &entries, &classification, &resolutions).unwrap();
        assert_eq!(added, 1); // entry 1 dropped (keep-existing), entry 2 kept (keep-new), entry 0 skipped
        let cleaned_entries = parse_entries(&cleaned).unwrap();
        assert_eq!(cleaned_entries.len(), 1);
        assert!(
            cleaned_entries
                .iter()
                .all(|e| e.url != "https://example.com/feed.xml")
        );
        assert!(
            cleaned_entries
                .iter()
                .any(|e| e.url == "http://example.org/b/")
        );
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
        let resolutions = vec![Resolution {
            key: 1,
            action: ResolutionAction::KeepNew,
            keep_existing_feed_id: None,
        }];
        let (cleaned, added) =
            build_cleaned_opml(opml, &entries, &classification, &resolutions).unwrap();
        assert_eq!(added, 1);
        let cleaned_entries = parse_entries(&cleaned).unwrap();
        assert_eq!(cleaned_entries.len(), 1);
        assert_eq!(cleaned_entries[0].title, "Second");
    }

    #[test]
    fn build_cleaned_opml_multiple_intra_file_keep_news_keep_the_last() {
        use crate::engine::opml_import::*;
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="One" title="One" type="rss" xmlUrl="https://example.com/dup"/>
    <outline text="Two" title="Two" type="rss" xmlUrl="https://example.com/dup/"/>
    <outline text="Three" title="Three" type="rss" xmlUrl="https://example.com/dup"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let classification = classify(&entries, &[]);
        // both later occurrences resolved keep-new; the latest (index 2) must win,
        // deterministically regardless of HashMap iteration order
        let resolutions = vec![
            Resolution {
                key: 1,
                action: ResolutionAction::KeepNew,
                keep_existing_feed_id: None,
            },
            Resolution {
                key: 2,
                action: ResolutionAction::KeepNew,
                keep_existing_feed_id: None,
            },
        ];
        let (cleaned, added) =
            build_cleaned_opml(opml, &entries, &classification, &resolutions).unwrap();
        assert_eq!(added, 1);
        let cleaned_entries = parse_entries(&cleaned).unwrap();
        assert_eq!(cleaned_entries.len(), 1);
        assert_eq!(cleaned_entries[0].title, "Three");
    }

    #[test]
    fn build_cleaned_opml_merges_sibling_categories_with_same_title() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example.com/a"/>
    </outline>
    <outline text="Tech" title="Tech">
      <outline text="Feed B" title="Feed B" type="rss" xmlUrl="https://example.com/b"/>
    </outline>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let classification = classify(&entries, &[]);
        let resolutions = vec![];
        let (cleaned, added) =
            build_cleaned_opml(opml, &entries, &classification, &resolutions).unwrap();
        assert_eq!(added, 2);
        let cleaned_entries = parse_entries(&cleaned).unwrap();
        let categories: Vec<&str> = cleaned_entries
            .iter()
            .map(|e| e.category.as_str())
            .collect();
        assert_eq!(categories, vec!["Tech", "Tech"]);
        let doc = opml::OPML::from_str(&cleaned).unwrap();
        let cats: Vec<String> = doc.body.outlines.iter().map(outline_title).collect();
        assert_eq!(
            cats,
            vec!["Tech"],
            "two sibling 'Tech' outlines must merge into one"
        );
        assert_eq!(
            doc.body.outlines[0].outlines.len(),
            2,
            "both feeds kept under the merged category"
        );
    }

    #[test]
    fn build_cleaned_opml_merges_nested_sibling_duplicates_but_not_across_parents() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Top" title="Top">
      <outline text="Sub" title="Sub">
        <outline text="A" title="A" type="rss" xmlUrl="https://example.com/a"/>
      </outline>
      <outline text="Sub" title="Sub">
        <outline text="B" title="B" type="rss" xmlUrl="https://example.com/b"/>
      </outline>
    </outline>
    <outline text="Other" title="Other">
      <outline text="Sub" title="Sub">
        <outline text="C" title="C" type="rss" xmlUrl="https://example.com/c"/>
      </outline>
    </outline>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let classification = classify(&entries, &[]);
        let (cleaned, added) = build_cleaned_opml(opml, &entries, &classification, &[]).unwrap();
        assert_eq!(added, 3);
        let doc = opml::OPML::from_str(&cleaned).unwrap();
        assert_eq!(
            doc.body.outlines.len(),
            2,
            "sibling 'Top'/'Other' outlines stay separate"
        );
        let top = &doc.body.outlines[0];
        let other = &doc.body.outlines[1];
        assert_eq!(outline_title(top), "Top");
        assert_eq!(outline_title(other), "Other");
        assert_eq!(
            top.outlines.len(),
            1,
            "nested sibling 'Sub' outlines merge into one"
        );
        assert_eq!(outline_title(&top.outlines[0]), "Sub");
        assert_eq!(
            top.outlines[0].outlines.len(),
            2,
            "A and B kept under the merged 'Sub'"
        );
        assert_eq!(
            other.outlines.len(),
            1,
            "'Sub' under 'Other' is a distinct category"
        );
        assert_eq!(
            other.outlines[0].outlines.len(),
            1,
            "C kept under 'Other'/'Sub'"
        );
    }

    #[test]
    fn build_cleaned_opml_does_not_merge_feeds_with_same_title() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Feed X" title="Feed X" type="rss" xmlUrl="https://example.com/x"/>
    <outline text="Feed X" title="Feed X" type="rss" xmlUrl="https://example.com/y"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let classification = classify(&entries, &[]);
        let (cleaned, added) = build_cleaned_opml(opml, &entries, &classification, &[]).unwrap();
        assert_eq!(added, 2);
        let cleaned_entries = parse_entries(&cleaned).unwrap();
        assert_eq!(
            cleaned_entries.len(),
            2,
            "same-title feeds with different URLs stay distinct"
        );
    }

    #[test]
    fn build_cleaned_opml_merges_sibling_categories_with_empty_resolved_title() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="" title="">
      <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example.com/a"/>
    </outline>
    <outline text="" title="">
      <outline text="Feed B" title="Feed B" type="rss" xmlUrl="https://example.com/b"/>
    </outline>
    <outline text="Feed X" title="Feed X" type="rss" xmlUrl="https://example.com/x"/>
    <outline text="Feed X" title="Feed X" type="rss" xmlUrl="https://example.com/y"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let classification = classify(&entries, &[]);
        let (cleaned, added) = build_cleaned_opml(opml, &entries, &classification, &[]).unwrap();
        assert_eq!(added, 4);
        let cleaned_entries = parse_entries(&cleaned).unwrap();
        assert_eq!(cleaned_entries.len(), 4);
        let doc = opml::OPML::from_str(&cleaned).unwrap();
        let cats: Vec<String> = doc.body.outlines.iter().map(outline_title).collect();
        assert_eq!(
            cats,
            vec!["", "Feed X", "Feed X"],
            "two sibling empty-title/empty-text outlines must merge into one"
        );
        assert_eq!(
            doc.body.outlines[0].outlines.len(),
            2,
            "both feeds kept under the merged empty-title category"
        );
        assert_eq!(
            doc.body.outlines[1].outlines.len(),
            0,
            "same-title feeds stay separate"
        );
        assert_eq!(
            doc.body.outlines[2].outlines.len(),
            0,
            "same-title feeds stay separate"
        );
    }
}
