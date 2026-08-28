//! Which query parameters are tracking and which are the link.

/// Removed from every host.
pub const EVERYWHERE: [&str; 31] = [
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "utm_name",
    "utm_reader",
    "utm_viz_id",
    "utm_pubreferrer",
    "fbclid",
    "gclid",
    "dclid",
    "gbraid",
    "wbraid",
    "msclkid",
    "yclid",
    "mc_cid",
    "mc_eid",
    "igshid",
    "twclid",
    "ttclid",
    "li_fat_id",
    "mkt_tok",
    "_hsenc",
    "_hsmi",
    "__twitter_impression",
    "fb_action_ids",
    "fb_action_types",
    "fb_source",
    "mibextid",
];

/// Campaign builders invent their own `utm_` names, so the family goes as a whole.
pub const EVERYWHERE_PREFIX: &str = "utm_";

/// Matched against the host and any of its subdomains, and added to the list above rather than
/// replacing it.
pub const PER_SITE: [(&str, &[&str]); 8] = [
    ("youtube.com", &["si", "pp", "feature", "kw"]),
    ("youtu.be", &["si", "pp", "feature", "kw"]),
    ("twitter.com", &["s", "t", "cn", "src", "refsrc", "ref_src", "ref_url"]),
    ("x.com", &["s", "t", "cn", "src", "refsrc", "ref_src", "ref_url"]),
    ("instagram.com", &["igsh"]),
    ("spotify.com", &["si"]),
    (
        "reddit.com",
        &[
            "correlation_id",
            "ref_campaign",
            "ref_source",
            "rdt",
            "share_id",
            "_branch_match_id",
            "$deep_link",
            "$3p",
            "$original_url",
        ],
    ),
    (
        "tiktok.com",
        &[
            "u_code",
            "preview_pb",
            "_d",
            "_t",
            "_r",
            "timestamp",
            "user_id",
            "share_app_name",
            "share_iid",
        ],
    ),
];

/// The user's own edits, kept as a difference from the lists above so a later release still
/// reaches somebody who has edited their rules.
#[derive(Clone, Debug, Default)]
pub struct Rules {
    pub added: Vec<String>,
    pub added_per_site: Vec<(String, String)>,
    pub disabled: Vec<String>,
}

impl Rules {
    pub fn removes(&self, host: &str, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        if self.disabled.iter().any(|off| off.eq_ignore_ascii_case(&name)) {
            return false;
        }
        if self.added.iter().any(|extra| extra.eq_ignore_ascii_case(&name)) {
            return true;
        }
        if self
            .added_per_site
            .iter()
            .any(|(site, extra)| extra.eq_ignore_ascii_case(&name) && covers(site, host))
        {
            return true;
        }

        built_in(host, &name)
    }
}

fn built_in(host: &str, name: &str) -> bool {
    if name.starts_with(EVERYWHERE_PREFIX) || EVERYWHERE.contains(&name) {
        return true;
    }
    PER_SITE.iter().filter(|(site, _)| covers(site, host)).any(|(_, names)| names.contains(&name))
}

/// A rule for a site covers its subdomains, so a link from `m.youtube.com` is cleaned too.
fn covers(site: &str, host: &str) -> bool {
    let host = host.trim_start_matches("www.").to_ascii_lowercase();
    host == site || host.ends_with(&format!(".{site}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_campaign_parameter_goes_from_any_site() {
        assert!(Rules::default().removes("example.org", "utm_source"));
        assert!(Rules::default().removes("example.org", "fbclid"));
    }

    #[test]
    fn a_campaign_name_nobody_listed_goes_too() {
        assert!(Rules::default().removes("example.org", "utm_whatever_they_invented"));
    }

    #[test]
    fn a_site_rule_reaches_its_subdomains_but_not_its_neighbours() {
        assert!(Rules::default().removes("youtube.com", "si"));
        assert!(Rules::default().removes("m.youtube.com", "si"));
        assert!(Rules::default().removes("www.youtube.com", "si"));
        assert!(!Rules::default().removes("notyoutube.com", "si"));
    }

    #[test]
    fn a_site_rule_stays_on_its_own_site() {
        assert!(!Rules::default().removes("example.org", "si"));
        assert!(Rules::default().removes("open.spotify.com", "si"));
    }

    #[test]
    fn names_match_whatever_their_capitals() {
        assert!(Rules::default().removes("example.org", "UTM_Source"));
    }

    #[test]
    fn a_rule_switched_off_stops_applying() {
        let rules = Rules { disabled: vec!["fbclid".into()], ..Rules::default() };
        assert!(!rules.removes("example.org", "fbclid"));
        assert!(rules.removes("example.org", "gclid"));
    }

    #[test]
    fn a_name_of_your_own_is_removed_everywhere() {
        let rules = Rules { added: vec!["ref".into()], ..Rules::default() };
        assert!(rules.removes("example.org", "ref"));
    }

    #[test]
    fn a_name_of_your_own_can_be_kept_to_one_site() {
        let rules = Rules {
            added_per_site: vec![("shop.example".into(), "aff".into())],
            ..Rules::default()
        };
        assert!(rules.removes("shop.example", "aff"));
        assert!(!rules.removes("other.example", "aff"));
    }

    #[test]
    fn switching_a_rule_off_beats_adding_it() {
        let rules =
            Rules { added: vec!["ref".into()], disabled: vec!["ref".into()], ..Rules::default() };
        assert!(!rules.removes("example.org", "ref"));
    }
}
