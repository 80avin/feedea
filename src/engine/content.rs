use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use scraper::node::{Element, Node};
use scraper::{Html, Selector};
use url::Url;

use html5ever::QualName;

/// Rewrite an article's HTML so every URL-bearing attribute is a fully
/// qualified http(s) URL resolved against `base_url`.
///
/// Some publishers embed absolute-path or relative URLs (e.g. `src="/img.png"`
/// or `src="img.png"`). Without qualification those resolve against feedea's
/// domain instead of the original site, breaking images and links in the reader.
/// Images are additionally proxied through `/img?u=` for privacy.
pub fn rewrite_html(html: &str, base_url: &str) -> String {
    let base = match Url::parse(base_url) {
        Ok(b) => b,
        Err(_) => return html.to_string(),
    };
    let mut doc = Html::parse_fragment(html);

    qualify_url_attrs(&mut doc, &base);
    qualify_srcset(&mut doc, &base);
    proxy_images(&mut doc, &base);

    doc.html()
}

/// Attributes that hold a single URL, qualified to an absolute http(s) URL.
/// `(selector, attribute)`.
const SINGLE_URL_ATTRS: &[(&str, &str)] = &[
    ("a[href]", "href"),
    ("area[href]", "href"),
    ("link[href]", "href"),
    ("source[src]", "src"),
    ("video[src]", "src"),
    ("video[poster]", "poster"),
    ("audio[src]", "src"),
    ("iframe[src]", "src"),
    ("embed[src]", "src"),
    ("object[data]", "data"),
    ("track[src]", "src"),
    ("input[src]", "src"),
    ("form[action]", "action"),
    ("use[href]", "href"),
    ("image[href]", "href"),
];

fn qualify_url_attrs(doc: &mut Html, base: &Url) {
    for (selector, attr) in SINGLE_URL_ATTRS {
        let sel = Selector::parse(selector).unwrap();
        let updates: Vec<_> = doc
            .select(&sel)
            .filter_map(|el| {
                let raw = el.value().attr(attr)?;
                let abs = resolve_url(base, raw)?;
                Some((el.id(), abs))
            })
            .collect();
        for (id, abs) in updates {
            if let Some(mut node) = doc.tree.get_mut(id)
                && let Node::Element(elem) = node.value()
            {
                set_attr(elem, attr, &abs);
            }
        }
    }
}

fn qualify_srcset(doc: &mut Html, base: &Url) {
    for selector in ["img[srcset]", "source[srcset]"] {
        let sel = Selector::parse(selector).unwrap();
        let updates: Vec<_> = doc
            .select(&sel)
            .filter_map(|el| {
                let raw = el.value().attr("srcset")?;
                let resolved = resolve_srcset(base, raw)?;
                Some((el.id(), resolved))
            })
            .collect();
        for (id, resolved) in updates {
            if let Some(mut node) = doc.tree.get_mut(id)
                && let Node::Element(elem) = node.value()
            {
                set_attr(elem, "srcset", &resolved);
            }
        }
    }
}

fn proxy_images(doc: &mut Html, base: &Url) {
    let img_sel = Selector::parse("img").unwrap();
    let updates: Vec<_> = doc
        .select(&img_sel)
        .filter_map(|img| {
            let src = img.value().attr("src")?;
            let abs = resolve_url(base, src)?;
            let encoded = percent_encode(abs.as_bytes(), NON_ALPHANUMERIC).to_string();
            Some((img.id(), abs, format!("/img?u={encoded}")))
        })
        .collect();
    for (id, abs, proxied) in updates {
        if let Some(mut node) = doc.tree.get_mut(id)
            && let Node::Element(elem) = node.value()
        {
            set_attr(elem, "data-original", &abs);
            set_attr(elem, "src", &proxied);
        }
    }
}

/// Resolve `raw` against `base` to a fully qualified URL. Returns `None` when
/// the value is empty, is a non-http(s) scheme (e.g. `data:`, `mailto:`,
/// `ftp:`), or fails to resolve — those are left untouched.
fn resolve_url(base: &Url, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // In-page fragment links must stay local, not become absolute URLs.
    if trimmed.starts_with('#') {
        return None;
    }
    if let Ok(parsed) = Url::parse(trimmed) {
        return (parsed.scheme() == "http" || parsed.scheme() == "https")
            .then(|| parsed.to_string());
    }
    base.join(trimmed).ok().map(|u| u.to_string())
}

/// Resolve a `srcset` attribute: each comma-separated candidate's URL is
/// qualified against `base`, keeping its descriptor (`1x`, `640w`, ...).
fn resolve_srcset(base: &Url, raw: &str) -> Option<String> {
    let mut out = Vec::new();
    let mut changed = false;
    for candidate in raw.split(',') {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let url = parts.next()?;
        let rest: Vec<&str> = parts.collect();
        if let Some(abs) = resolve_url(base, url) {
            changed = true;
            let mut entry = abs;
            for descriptor in rest {
                entry.push(' ');
                entry.push_str(descriptor);
            }
            out.push(entry);
        } else {
            out.push(trimmed.to_string());
        }
    }
    if !changed {
        return None;
    }
    Some(out.join(", "))
}

fn set_attr(elem: &mut Element, name: &str, value: &str) {
    let local = html5ever::LocalName::from(name);
    if let Some((_, v)) = elem.attrs.iter_mut().find(|(qn, _)| qn.local == local) {
        *v = scraper::StrTendril::from(value);
    } else {
        elem.attrs.push((
            QualName::new(None, html5ever::Namespace::from(""), local),
            scraper::StrTendril::from(value),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://example.com/articles/some-post";

    #[test]
    fn relative_link_becomes_absolute() {
        let out = rewrite_html(r#"<p><a href="/rel">link</a></p>"#, BASE);
        assert!(
            out.contains(r#"href="https://example.com/rel""#),
            "got: {out}"
        );
    }

    #[test]
    fn absolute_image_is_proxied_and_preserves_original() {
        let out = rewrite_html(r#"<img src="https://x/y.png">"#, BASE);
        let encoded = percent_encode(b"https://x/y.png", NON_ALPHANUMERIC);
        assert!(
            out.contains(&format!("src=\"/img?u={encoded}\"")),
            "got: {out}"
        );
        assert!(
            out.contains(r#"data-original="https://x/y.png""#),
            "got: {out}"
        );
    }

    #[test]
    fn relative_image_src_resolves() {
        let out = rewrite_html(r#"<img src="/img.png">"#, BASE);
        let encoded = percent_encode(b"https://example.com/img.png", NON_ALPHANUMERIC);
        assert!(
            out.contains(&format!("src=\"/img?u={encoded}\"")),
            "got: {out}"
        );
        assert!(
            out.contains(r#"data-original="https://example.com/img.png""#),
            "got: {out}"
        );
    }

    #[test]
    fn data_uri_image_is_not_proxied() {
        let out = rewrite_html(r#"<img src="data:image/png;base64,AAAA">"#, BASE);
        assert!(
            out.contains(r#"src="data:image/png;base64,AAAA""#),
            "got: {out}"
        );
        assert!(!out.contains("/img?u="), "got: {out}");
        assert!(!out.contains("data-original"), "got: {out}");
    }

    #[test]
    fn non_http_scheme_image_is_not_proxied() {
        let out = rewrite_html(r#"<img src="ftp://example.com/x.png">"#, BASE);
        assert!(
            out.contains(r#"src="ftp://example.com/x.png""#),
            "got: {out}"
        );
        assert!(!out.contains("/img?u="), "got: {out}");
        assert!(!out.contains("data-original"), "got: {out}");
    }

    #[test]
    fn external_absolute_link_stays_absolute() {
        let out = rewrite_html(r#"<a href="https://external.org/page">ext</a>"#, BASE);
        assert!(
            out.contains(r#"href="https://external.org/page""#),
            "got: {out}"
        );
    }

    #[test]
    fn invalid_base_returns_input_unchanged() {
        let html = r#"<a href="/rel">link</a>"#;
        assert_eq!(rewrite_html(html, "not a url"), html);
    }

    #[test]
    fn source_tag_relative_src_is_qualified() {
        let out = rewrite_html(
            r#"<video controls><source src="/media/clip.mp4" type="video/mp4"></video>"#,
            BASE,
        );
        assert!(
            out.contains(r#"src="https://example.com/media/clip.mp4""#),
            "got: {out}"
        );
    }

    #[test]
    fn video_poster_relative_is_qualified() {
        let out = rewrite_html(
            r#"<video poster="/img/poster.jpg" src="clip.mp4"></video>"#,
            BASE,
        );
        assert!(
            out.contains(r#"poster="https://example.com/img/poster.jpg""#),
            "got: {out}"
        );
        assert!(
            out.contains(r#"src="https://example.com/articles/clip.mp4""#),
            "got: {out}"
        );
    }

    #[test]
    fn iframe_relative_src_is_qualified() {
        let out = rewrite_html(r#"<iframe src="/embed/thing"></iframe>"#, BASE);
        assert!(
            out.contains(r#"src="https://example.com/embed/thing""#),
            "got: {out}"
        );
    }

    #[test]
    fn object_data_relative_is_qualified() {
        let out = rewrite_html(r#"<object data="/files/doc.pdf"></object>"#, BASE);
        assert!(
            out.contains(r#"data="https://example.com/files/doc.pdf""#),
            "got: {out}"
        );
    }

    #[test]
    fn form_action_relative_is_qualified() {
        let out = rewrite_html(
            r#"<form action="/submit"><input src="/i.png"></form>"#,
            BASE,
        );
        assert!(
            out.contains(r#"action="https://example.com/submit""#),
            "got: {out}"
        );
        assert!(
            out.contains(r#"src="https://example.com/i.png""#),
            "got: {out}"
        );
    }

    #[test]
    fn protocol_relative_url_gets_scheme() {
        let out = rewrite_html(r#"<a href="//cdn.example.com/x">cdn</a>"#, BASE);
        assert!(
            out.contains(r#"href="https://cdn.example.com/x""#),
            "got: {out}"
        );
    }

    #[test]
    fn mailto_and_anchor_links_are_untouched() {
        let out = rewrite_html(
            "<a href=\"mailto:a@b.com\">mail</a><a href=\"#sec\">anchor</a>",
            BASE,
        );
        assert!(out.contains(r#"href="mailto:a@b.com""#), "got: {out}");
        assert!(out.contains("href=\"#sec\""), "got: {out}");
    }

    #[test]
    fn srcset_relative_urls_are_qualified_with_descriptors() {
        let out = rewrite_html(r#"<img srcset="/a.png 1x, /b.png 2x" src="/a.png">"#, BASE);
        assert!(
            out.contains(r#"srcset="https://example.com/a.png 1x, https://example.com/b.png 2x""#),
            "got: {out}"
        );
    }
}
