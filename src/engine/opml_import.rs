use std::collections::{HashMap, HashSet};

use opml::{OPML, Outline};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct OpmlEntry {
    pub index: usize,
    pub title: String,
    pub url: String,
    pub category: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExistingFeed {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub website: Option<String>,
    pub category: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    /// Same feed (matching normalized url) whose title or category differs.
    SameFeed,
    /// Duplicate of another entry within the same file.
    IntraFile,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub key: usize,
    pub kind: ConflictKind,
    pub opml: OpmlEntry,
    pub matches: Vec<ExistingFeed>,
    /// How many entries in this file share the entry's normalized url (>= 1).
    /// Lets the UI surface that a source appears more than once in the file.
    pub occurrences: usize,
}

#[derive(Debug, Clone)]
pub struct Classification {
    pub conflicts: Vec<Conflict>,
    pub new_count: usize,
    pub exact_duplicates: usize,
    pub skipped: HashSet<usize>,
}

fn collect(
    outlines: &[Outline],
    category: &Vec<String>,
    index: &mut usize,
    out: &mut Vec<OpmlEntry>,
) {
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
                category: category.clone(),
            });
            *index += 1;
        } else {
            let title = outline
                .title
                .clone()
                .filter(|t| !t.is_empty())
                .or_else(|| Some(outline.text.clone()))
                .unwrap_or_default();
            if title.is_empty() {
                collect(&outline.outlines, category, index, out);
            } else {
                let mut cat = category.clone();
                cat.push(title);
                collect(&outline.outlines, &cat, index, out);
            };
        }
    }
}

pub fn parse_entries(opml_str: &str) -> anyhow::Result<Vec<OpmlEntry>> {
    let doc = OPML::from_str(opml_str).map_err(|e| anyhow::anyhow!("invalid opml: {e}"))?;
    let mut entries = Vec::new();
    let mut index = 0;
    collect(&doc.body.outlines, &Vec::new(), &mut index, &mut entries);
    Ok(entries)
}

pub fn normalize_url(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw).ok()?;
    let scheme = parsed.scheme().to_lowercase();
    let host = parsed.host_str()?.to_lowercase();
    let mut normalized = format!("{scheme}://{host}");
    if let Some(port) = parsed.port() {
        let is_default = (scheme == "http" && port == 80) || (scheme == "https" && port == 443);
        if !is_default {
            normalized.push_str(&format!(":{port}"));
        }
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

/// Full category path (root-most first) for every category, keyed by category id.
///
/// Walks `CategoryMapping` parents upward from each category until the toplevel
/// pseudo-category. The toplevel id itself is excluded, so an uncategorized feed
/// (whose `category_id` is the toplevel id) looks up an empty path.
pub fn category_paths(
    categories: &[news_flash::models::Category],
    mappings: &[news_flash::models::CategoryMapping],
) -> HashMap<String, Vec<String>> {
    let name_by_id: HashMap<&str, &str> = categories
        .iter()
        .map(|c| (c.category_id.as_str(), c.label.as_str()))
        .collect();
    let parent_by_id: HashMap<&str, &str> = mappings
        .iter()
        .map(|m| (m.category_id.as_str(), m.parent_id.as_str()))
        .collect();
    let toplevel = news_flash::models::NEWSFLASH_TOPLEVEL.as_str();

    categories
        .iter()
        .filter(|c| c.category_id.as_str() != toplevel)
        .map(|c| {
            let mut path = Vec::new();
            let mut cur = c.category_id.as_str();
            let mut seen = HashSet::new();
            loop {
                if !seen.insert(cur) {
                    // corrupt parent chain (cycle); keep what we have
                    break;
                }
                match parent_by_id.get(cur) {
                    Some(parent) if *parent != toplevel => {
                        if let Some(name) = name_by_id.get(cur) {
                            path.push((*name).to_string());
                        }
                        cur = parent;
                    }
                    _ => {
                        if let Some(name) = name_by_id.get(cur) {
                            path.push((*name).to_string());
                        }
                        break;
                    }
                }
            }
            path.reverse();
            (c.category_id.as_str().to_string(), path)
        })
        .collect()
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

    // Count how many file entries share each normalized url so the UI can show that a
    // source appears more than once in the file.
    let mut url_count: HashMap<String, usize> = HashMap::new();
    for entry in entries {
        if let Some(n) = normalize_url(&entry.url) {
            *url_count.entry(n).or_insert(0) += 1;
        }
    }
    let occurrences_of = |norm: &Option<String>| {
        norm.as_ref()
            .and_then(|n| url_count.get(n))
            .copied()
            .unwrap_or(1)
    };

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

        // A genuine url identity match (raw id equality or url-normalized feed_url) is
        // what decides "same feed". An existing feed whose *website* equals the entry url
        // is a different feed per that model, so a website-only match must always surface
        // as a conflict for the user to decide, never be silently skipped.
        let has_url_match = norm.as_ref().is_some_and(|n| {
            matches.iter().any(|f| {
                f.id == entry.url || f.url.as_deref().and_then(normalize_url).as_ref() == Some(n)
            })
        });

        // The normalized url decides whether two feeds are the same feed. Pick the most
        // representative existing match for the detail comparison: an id match (raw url
        // equality) first, then a feed whose actual url normalizes to the same value,
        // then any remaining match (e.g. a website match).
        let primary = matches
            .iter()
            .find(|f| f.id == entry.url)
            .or_else(|| {
                norm.as_ref().and_then(|n| {
                    matches
                        .iter()
                        .find(|f| f.url.as_deref().and_then(normalize_url).as_ref() == Some(n))
                })
            })
            .or_else(|| matches.first());

        if let Some(primary) = primary {
            if has_url_match && primary.title == entry.title && primary.category == entry.category {
                skipped.insert(entry.index);
            } else {
                conflicts.push(Conflict {
                    key: entry.index,
                    kind: ConflictKind::SameFeed,
                    opml: entry.clone(),
                    matches: matches.clone(),
                    occurrences: occurrences_of(&norm),
                });
            }
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
                    occurrences: occurrences_of(&norm),
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
                ConflictKind::SameFeed => {
                    // A raw-url-identical conflict is resolved by rename/move in the
                    // handler and is never imported via the opml; a url variant that
                    // is kept-new is imported (the handler migrates + removes the old).
                    let has_id_match = conflict.matches.iter().any(|m| m.id == conflict.opml.url);
                    if keep_new.contains(&entry.index) && !has_id_match {
                        keep.insert(entry.index);
                    }
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
                category: vec!["Tech".to_string()],
            },
            ExistingFeed {
                id: "http://example.org/b".to_string(),
                title: "Feed B".to_string(),
                url: Some("http://example.org/b".to_string()),
                website: None,
                category: Vec::new(),
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
        assert_eq!(entries[0].category, vec!["Tech".to_string()]);
        assert_eq!(entries[1].index, 1);
        assert_eq!(entries[1].category, vec!["Tech".to_string()]);
        assert_eq!(entries[2].index, 2);
        assert_eq!(entries[2].category, Vec::<String>::new());
    }

    #[test]
    fn normalize_url_strips_trailing_slash_fragment_and_default_port() {
        assert_eq!(
            normalize_url("HTTP://Example.COM:443/feed.xml/"),
            normalize_url("http://example.com:443/feed.xml#frag"),
            "case and fragment differ; :443 is non-default for http so it is kept"
        );
        assert_eq!(
            normalize_url("http://example.com:80/feed.xml"),
            normalize_url("http://example.com/feed.xml")
        );
        assert_eq!(
            normalize_url("https://example.com:443/feed.xml"),
            normalize_url("https://example.com/feed.xml")
        );
        assert_eq!(
            normalize_url("https://example.com:8443/feed.xml"),
            Some("https://example.com:8443/feed.xml".into()),
            "non-default ports are preserved"
        );
        assert_eq!(
            normalize_url("https://example.com"),
            Some("https://example.com".into())
        );
        assert_eq!(normalize_url("not a url"), None);
    }

    #[test]
    fn parse_entries_builds_nested_category_path() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Top" title="Top">
      <outline text="Sub" title="Sub">
        <outline text="Feed" title="Feed" type="rss" xmlUrl="https://example.com/deep"/>
      </outline>
      <outline text="Feed 2" title="Feed 2" type="rss" xmlUrl="https://example.com/shallow"/>
    </outline>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        assert_eq!(
            entries[0].category,
            vec!["Top".to_string(), "Sub".to_string()]
        );
        assert_eq!(entries[1].category, vec!["Top".to_string()]);
    }

    #[test]
    fn classifies_url_matches_by_normalized_url_and_skips_same_details() {
        // Feed A exists with the same normalized url but a raw-url variant
        // (trailing slash). Same title and category -> same feed -> skipped.
        let mut existing = existing();
        existing[0].title = "Renamed".to_string();
        let entries = parse_entries(OPML).unwrap();
        let c = classify(&entries, &existing);
        assert_eq!(c.new_count, 0);
        assert_eq!(c.exact_duplicates, 1);
        assert!(c.skipped.contains(&2));
        // entry 0: raw url equals Feed A's id, title differs -> SameFeed
        // entry 1: trailing-slash variant of Feed A's url, title differs -> SameFeed
        // entry 2: trailing-slash variant of Feed B's url, all details same -> skipped
        let conflicts: Vec<&Conflict> = c.conflicts.iter().collect();
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts.iter().all(|x| x.kind == ConflictKind::SameFeed));
        let a = c.conflicts.iter().find(|x| x.key == 0).unwrap();
        assert!(
            a.matches
                .iter()
                .any(|m| m.id == "https://example.com/feed.xml")
        );
        let a_variant = c.conflicts.iter().find(|x| x.key == 1).unwrap();
        assert!(
            a_variant
                .matches
                .iter()
                .any(|m| m.id == "https://example.com/feed.xml")
        );
    }

    #[test]
    fn website_only_match_is_never_skipped() {
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example.com/site"/>
  </body>
</opml>"#;
        let existing = vec![ExistingFeed {
            id: "https://example.com/feed.xml".to_string(),
            title: "Feed A".to_string(),
            url: Some("https://example.com/feed.xml".to_string()),
            website: Some("https://example.com/site".to_string()),
            category: vec!["Tech".to_string()],
        }];
        let entries = parse_entries(opml).unwrap();
        let c = classify(&entries, &existing);
        // Identical title and category, but the entry's feed_url is NOT the existing
        // feed's url: only a website match. That must surface as a conflict so the
        // entry is not silently dropped.
        assert_eq!(c.exact_duplicates, 0);
        assert_eq!(c.conflicts.len(), 1);
        assert_eq!(c.conflicts[0].kind, ConflictKind::SameFeed);
    }

    #[test]
    fn conflict_reports_occurrences_of_duplicated_source() {
        // Same source twice in one file (same normalized url, different categories).
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="prog" title="prog">
      <outline text="Sigplan" title="Sigplan" type="rss" xmlUrl="https://blog.sigplan.org/feed/"/>
    </outline>
    <outline text="langs" title="langs">
      <outline text="Sigplan Blog" title="Sigplan Blog" type="rss" xmlUrl="https://blog.sigplan.org/feed/"/>
    </outline>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let c = classify(&entries, &[]);
        // Second occurrence conflicts (intra-file); the first is new. The conflict
        // must report that its source appears twice in the file.
        assert_eq!(c.conflicts.len(), 1);
        assert_eq!(c.conflicts[0].occurrences, 2);
    }

    #[test]
    fn category_paths_walks_parents_root_first() {
        use news_flash::models::{Category, CategoryID, CategoryMapping};
        let toplevel = news_flash::models::NEWSFLASH_TOPLEVEL.as_str().to_string();
        let top = CategoryID::new("top");
        let sub = CategoryID::new("sub");
        let leaf = CategoryID::new("leaf");
        let categories = vec![
            Category {
                category_id: top.clone(),
                label: "Top".into(),
            },
            Category {
                category_id: sub.clone(),
                label: "Sub".into(),
            },
            Category {
                category_id: leaf.clone(),
                label: "Leaf".into(),
            },
        ];
        let mappings = vec![
            CategoryMapping {
                parent_id: CategoryID::new(&toplevel),
                category_id: top.clone(),
                sort_index: None,
            },
            CategoryMapping {
                parent_id: top.clone(),
                category_id: sub.clone(),
                sort_index: None,
            },
            CategoryMapping {
                parent_id: sub.clone(),
                category_id: leaf.clone(),
                sort_index: None,
            },
        ];
        let paths = category_paths(&categories, &mappings);
        assert_eq!(
            paths.get(leaf.as_str()).unwrap(),
            &vec!["Top".to_string(), "Sub".to_string(), "Leaf".to_string()]
        );
        assert_eq!(
            paths.get(sub.as_str()).unwrap(),
            &vec!["Top".to_string(), "Sub".to_string()]
        );
        assert_eq!(paths.get(top.as_str()).unwrap(), &vec!["Top".to_string()]);
        assert!(
            !paths.contains_key(&toplevel),
            "toplevel id is excluded so uncategorized feeds map to an empty path"
        );
    }

    #[test]
    fn category_paths_breaks_cycles() {
        use news_flash::models::{Category, CategoryID, CategoryMapping};
        let a = CategoryID::new("a");
        let b = CategoryID::new("b");
        let categories = vec![
            Category {
                category_id: a.clone(),
                label: "A".into(),
            },
            Category {
                category_id: b.clone(),
                label: "B".into(),
            },
        ];
        let mappings = vec![
            CategoryMapping {
                parent_id: b.clone(),
                category_id: a.clone(),
                sort_index: None,
            },
            CategoryMapping {
                parent_id: a.clone(),
                category_id: b.clone(),
                sort_index: None,
            },
        ];
        let paths = category_paths(&categories, &mappings);
        // no infinite loop; each category still gets a path containing its own label
        assert_eq!(paths.len(), 2);
        assert!(paths.get(a.as_str()).unwrap().contains(&"A".to_string()));
        assert!(paths.get(b.as_str()).unwrap().contains(&"B".to_string()));
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
        // entry 2 matches Feed B by normalized url with identical details -> skipped
        assert_eq!(c.exact_duplicates, 2);
        assert!(c.skipped.contains(&0));
        assert!(c.skipped.contains(&2));
        // entry 1: trailing-slash variant of Feed A's url with a different title -> conflict
        assert_eq!(c.conflicts.len(), 1);
        assert_eq!(c.conflicts[0].key, 1);
        assert_eq!(c.conflicts[0].kind, ConflictKind::SameFeed);
        assert_eq!(c.new_count, 0);
    }

    #[test]
    fn classify_conflicts_when_nested_category_path_differs() {
        let mut existing = existing();
        existing[0].category = vec!["Top".to_string(), "Sub".to_string()];
        existing[0].url = Some("https://example.com/feed.xml/".to_string());
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Top" title="Top">
      <outline text="Sub" title="Sub">
        <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example.com/feed.xml"/>
      </outline>
    </outline>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        // same url, same title, same full category path -> skipped
        let c = classify(&entries, &existing);
        assert_eq!(c.exact_duplicates, 1);
        assert!(c.skipped.contains(&0));
        assert_eq!(c.conflicts.len(), 0);

        // same url + title, but a different nested path -> SameFeed conflict
        existing[0].category = vec!["Other".to_string(), "Sub".to_string()];
        let c = classify(&entries, &existing);
        assert_eq!(c.exact_duplicates, 0);
        assert_eq!(c.conflicts.len(), 1);
        assert_eq!(c.conflicts[0].kind, ConflictKind::SameFeed);

        // same url + title, leaf name matches but parent differs -> SameFeed conflict
        existing[0].category = vec!["Sub".to_string()];
        let c = classify(&entries, &existing);
        assert_eq!(c.exact_duplicates, 0);
        assert_eq!(c.conflicts.len(), 1);
        assert_eq!(c.conflicts[0].kind, ConflictKind::SameFeed);
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
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed A" title="Feed A" type="rss" xmlUrl="https://example.com/feed.xml"/>
      <outline text="Feed A Again" title="Feed A Again" type="rss" xmlUrl="https://example.com/feed.xml/"/>
    </outline>
    <outline text="Feed B" title="Feed B Renamed" type="rss" xmlUrl="http://example.org/b/"/>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let classification = classify(&entries, &existing());
        // entry 0 is an exact duplicate (skipped). Entries 1 and 2 are same-feed
        // conflicts (title differs). Entry 2 is also a raw-url variant, so keep-new
        // imports it.
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
            build_cleaned_opml(opml, &entries, &classification, &resolutions).unwrap();
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
    fn build_cleaned_opml_never_imports_raw_url_identical_conflict() {
        use crate::engine::opml_import::*;
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed A" title="Feed A Renamed" type="rss" xmlUrl="https://example.com/feed.xml"/>
    </outline>
  </body>
</opml>"#;
        let entries = parse_entries(opml).unwrap();
        let classification = classify(&entries, &existing());
        // raw url identical to existing Feed A's id, title differs -> SameFeed conflict
        assert_eq!(classification.conflicts.len(), 1);
        let resolutions = vec![Resolution {
            key: 0,
            action: ResolutionAction::KeepNew,
            keep_existing_feed_id: None,
        }];
        let (cleaned, added) =
            build_cleaned_opml(opml, &entries, &classification, &resolutions).unwrap();
        assert_eq!(
            added, 0,
            "raw-url-identical conflicts are resolved by rename/move, never imported"
        );
        assert_eq!(parse_entries(&cleaned).unwrap().len(), 0);
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
        let categories: Vec<Vec<_>> = cleaned_entries.iter().map(|e| e.category.clone()).collect();
        assert_eq!(
            categories,
            vec![vec!["Tech".to_string()], vec!["Tech".to_string()]]
        );
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
