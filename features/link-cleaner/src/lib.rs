//! Taking the tracking out of a copied link.

pub mod rules;

pub use rules::{CAMPAIGN_FAMILY, RuleDefinition, RuleError, Rules};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cleaned {
    pub link: String,
    pub removed: Vec<String>,
}

/// Nothing back when the text is not a single web link, or when there was nothing to take out.
///
/// The query is rebuilt from the original text rather than from parsed pairs, so a link that
/// keeps every parameter it had comes out byte for byte as it went in. Re-encoding it would
/// turn plus signs into spaces and spaces into plus signs on links nobody asked us to touch.
pub fn clean(text: &str, rules: &Rules) -> Option<Cleaned> {
    let trimmed = text.trim();
    let mut link = url::Url::parse(trimmed).ok()?;

    if !matches!(link.scheme(), "http" | "https") {
        return None;
    }
    let host = link.host_str()?.to_owned();
    let query = link.query()?;

    let mut kept = Vec::new();
    let mut removed = Vec::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let name = pair.split('=').next().unwrap_or(pair);
        if rules.removes(&host, name) {
            removed.push(name.to_ascii_lowercase());
        } else {
            kept.push(pair);
        }
    }

    if removed.is_empty() {
        return None;
    }

    // An emptied query leaves a trailing question mark behind unless it is unset outright.
    link.set_query((!kept.is_empty()).then(|| kept.join("&")).as_deref());
    Some(Cleaned { link: link.to_string(), removed })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleaning(text: &str) -> Option<Cleaned> {
        clean(text, &Rules::default())
    }

    #[test]
    fn a_campaign_link_loses_its_campaign() {
        let cleaned = cleaning("https://example.org/read?utm_source=news&id=7").unwrap();
        assert_eq!(cleaned.link, "https://example.org/read?id=7");
        assert_eq!(cleaned.removed, vec!["utm_source"]);
    }

    #[test]
    fn a_link_that_was_only_tracking_keeps_no_question_mark() {
        let cleaned = cleaning("https://example.org/read?fbclid=abc").unwrap();
        assert_eq!(cleaned.link, "https://example.org/read");
    }

    #[test]
    fn a_clean_link_is_left_exactly_as_it_was() {
        assert!(cleaning("https://example.org/read?id=7&q=a+b%20c").is_none());
    }

    #[test]
    fn what_is_kept_is_not_re_encoded() {
        let cleaned = cleaning("https://example.org/s?q=a+b%20c&utm_id=9").unwrap();
        assert_eq!(cleaned.link, "https://example.org/s?q=a+b%20c");
    }

    #[test]
    fn a_youtube_link_keeps_the_video_and_loses_the_referrer() {
        let cleaned = cleaning("https://youtu.be/dQw4w9WgXcQ?si=abc123&t=42").unwrap();
        assert_eq!(cleaned.link, "https://youtu.be/dQw4w9WgXcQ?t=42");
    }

    #[test]
    fn ordinary_text_is_not_a_link() {
        assert!(cleaning("meet me at the usual place").is_none());
        assert!(cleaning("example.org?utm_source=news").is_none());
    }

    #[test]
    fn only_the_web_is_cleaned() {
        assert!(cleaning("ftp://example.org/f?utm_source=news").is_none());
        assert!(cleaning("mailto:someone@example.org?utm_source=news").is_none());
    }

    #[test]
    fn a_link_with_no_query_has_nothing_to_take_out() {
        assert!(cleaning("https://example.org/read").is_none());
    }

    #[test]
    fn the_fragment_is_left_alone() {
        let cleaned = cleaning("https://example.org/read?utm_id=1#part-two").unwrap();
        assert_eq!(cleaned.link, "https://example.org/read#part-two");
    }

    #[test]
    fn a_rule_of_your_own_is_honoured() {
        let rules = Rules { added: vec!["ref".into()], ..Rules::default() };
        let cleaned = clean("https://example.org/p?ref=twitter&id=1", &rules).unwrap();
        assert_eq!(cleaned.link, "https://example.org/p?id=1");
    }

    #[test]
    fn surrounding_space_does_not_stop_it() {
        assert!(cleaning("  https://example.org/p?utm_id=1  ").is_some());
    }
}
