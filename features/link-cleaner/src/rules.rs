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
/// reaches somebody who has edited their rules. Every entry is a normalised name; `disabled`
/// holds rule identities, `scope<TAB>parameter` with `*` for every site.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rules {
    pub added: Vec<String>,
    pub added_per_site: Vec<(String, String)>,
    pub disabled: Vec<String>,
}

/// The campaign family as one visible rule. It stands in for every `utm_` name nobody listed,
/// so the listed ones keep their own switch.
pub const CAMPAIGN_FAMILY: &str = "utm_*";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleDefinition {
    /// Empty means every site.
    pub scope: String,
    pub parameter: String,
    pub enabled: bool,
    pub custom: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    EmptyParameter,
    InvalidParameter,
    InvalidDomain,
    AlreadyListed,
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str(match self {
            Self::EmptyParameter => "Enter a parameter name",
            Self::InvalidParameter => "A parameter name has no spaces, ?, &, = or #",
            Self::InvalidDomain => "Enter a bare domain such as example.org",
            Self::AlreadyListed => "That rule is already in the list",
        })
    }
}

impl Rules {
    /// A name goes when a rule for it covers the host and none of those rules is switched
    /// off, so switching one off for a site beats a global rule for the same name. The family
    /// rule only answers for `utm_` names that have no rule of their own.
    pub fn removes(&self, host: &str, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        let named: Vec<&str> = self
            .catalogue()
            .iter()
            .filter(|rule| rule.parameter == name && covers(&rule.scope, host))
            .map(|rule| if rule.enabled { "on" } else { "off" })
            .collect();
        if !named.is_empty() {
            return named.iter().all(|state| *state == "on");
        }
        name.starts_with(EVERYWHERE_PREFIX) && !self.is_disabled("", CAMPAIGN_FAMILY)
    }

    pub fn add(&mut self, scope: &str, parameter: &str) -> Result<(), RuleError> {
        let scope = normalise_domain(scope)?;
        let parameter = normalise_parameter(parameter)?;
        if self.catalogue().iter().any(|rule| rule.scope == scope && rule.parameter == parameter) {
            return Err(RuleError::AlreadyListed);
        }
        if scope.is_empty() {
            self.added.push(parameter);
        } else {
            self.added_per_site.push((scope, parameter));
        }
        Ok(())
    }

    /// Only the user's own rules can go; a built-in one is switched off instead.
    pub fn remove(&mut self, scope: &str, parameter: &str) {
        self.added.retain(|known| !(scope.is_empty() && known == parameter));
        self.added_per_site.retain(|(site, known)| !(site == scope && known == parameter));
        let identity = identity(scope, parameter);
        self.disabled.retain(|known| known != &identity);
    }

    pub fn set_enabled(&mut self, scope: &str, parameter: &str, enabled: bool) {
        let identity = identity(scope, parameter);
        self.disabled.retain(|known| known != &identity);
        if !enabled {
            self.disabled.push(identity);
        }
    }

    /// Every rule there is, the family first, then every site's names, then each site in turn.
    pub fn catalogue(&self) -> Vec<RuleDefinition> {
        let mut rules = vec![self.definition("", CAMPAIGN_FAMILY, false)];
        rules.extend(EVERYWHERE.iter().map(|parameter| self.definition("", parameter, false)));
        rules.extend(self.added.iter().map(|parameter| self.definition("", parameter, true)));
        for (scope, parameters) in PER_SITE {
            rules.extend(
                parameters.iter().map(|parameter| self.definition(scope, parameter, false)),
            );
        }
        rules.extend(
            self.added_per_site
                .iter()
                .map(|(scope, parameter)| self.definition(scope, parameter, true)),
        );
        rules.sort_by(|left, right| {
            let family = |rule: &RuleDefinition| rule.parameter != CAMPAIGN_FAMILY;
            (left.scope.as_str(), family(left), left.parameter.trim_start_matches('_')).cmp(&(
                right.scope.as_str(),
                family(right),
                right.parameter.trim_start_matches('_'),
            ))
        });
        rules
    }

    pub fn added_entries(&self) -> Vec<String> {
        self.added
            .iter()
            .map(|parameter| identity("", parameter))
            .chain(self.added_per_site.iter().map(|(scope, parameter)| identity(scope, parameter)))
            .collect()
    }

    /// Anything on disk that does not decode is dropped rather than kept as a rule that can
    /// never match.
    pub fn from_entries(added: &[String], disabled: &[String]) -> Self {
        let mut rules = Self::default();
        for (scope, parameter) in added.iter().filter_map(|entry| decode_identity(entry)) {
            let _ = rules.add(&scope, &parameter);
        }
        rules.disabled =
            disabled.iter().filter(|entry| decode_identity(entry).is_some()).cloned().collect();
        rules
    }

    fn definition(&self, scope: &str, parameter: &str, custom: bool) -> RuleDefinition {
        RuleDefinition {
            scope: scope.to_owned(),
            parameter: parameter.to_owned(),
            enabled: !self.is_disabled(scope, parameter),
            custom,
        }
    }

    fn is_disabled(&self, scope: &str, parameter: &str) -> bool {
        self.disabled.contains(&identity(scope, parameter))
    }
}

/// A rule for a site covers its subdomains, so a link from `m.youtube.com` is cleaned too.
/// The empty scope covers everything.
fn covers(site: &str, host: &str) -> bool {
    let host = host.trim_start_matches("www.").to_ascii_lowercase();
    site.is_empty() || host == site || host.ends_with(&format!(".{site}"))
}

fn normalise_parameter(parameter: &str) -> Result<String, RuleError> {
    let parameter = parameter.trim().to_ascii_lowercase();
    if parameter.is_empty() {
        return Err(RuleError::EmptyParameter);
    }
    if parameter.chars().any(|character| character.is_whitespace() || "?&=#".contains(character)) {
        return Err(RuleError::InvalidParameter);
    }
    Ok(parameter)
}

fn normalise_domain(domain: &str) -> Result<String, RuleError> {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty() {
        return Ok(String::new());
    }
    let shaped = !domain.contains(['/', '?', '&', '=', '#', ':'])
        && !domain.chars().any(char::is_whitespace)
        && domain.contains('.');
    if !shaped {
        return Err(RuleError::InvalidDomain);
    }
    Ok(domain.trim_start_matches("www.").to_owned())
}

fn identity(scope: &str, parameter: &str) -> String {
    format!("{}\t{}", if scope.is_empty() { "*" } else { scope }, parameter)
}

fn decode_identity(entry: &str) -> Option<(String, String)> {
    let (scope, parameter) = entry.split_once('\t')?;
    let scope = if scope == "*" { "" } else { scope };
    Some((normalise_domain(scope).ok()?, normalise_parameter(parameter).ok()?))
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
    fn the_visible_campaign_family_rule_can_be_disabled() {
        let mut rules = Rules::default();
        rules.set_enabled("", "utm_*", false);
        assert!(!rules.removes("example.org", "utm_whatever_they_invented"));
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
        let rules = Rules { disabled: vec![identity("", "fbclid")], ..Rules::default() };
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
        let rules = Rules {
            added: vec!["ref".into()],
            disabled: vec![identity("", "ref")],
            ..Rules::default()
        };
        assert!(!rules.removes("example.org", "ref"));
    }

    #[test]
    fn the_same_name_is_disabled_per_site() {
        let mut rules = Rules::default();
        rules.set_enabled("youtube.com", "si", false);
        assert!(!rules.removes("youtube.com", "si"));
        assert!(rules.removes("spotify.com", "si"));
    }

    #[test]
    fn custom_rules_are_normalised_and_round_trip_through_settings_entries() {
        let mut rules = Rules::default();
        rules.add(" WWW.Example.org. ", " Affiliate ").unwrap();
        rules.set_enabled("example.org", "affiliate", false);
        let restored = Rules::from_entries(&rules.added_entries(), &rules.disabled);
        assert_eq!(restored.added_per_site, vec![("example.org".into(), "affiliate".into())]);
        assert!(!restored.removes("shop.example.org", "affiliate"));
    }

    #[test]
    fn empty_and_query_shaped_rules_are_rejected() {
        assert_eq!(normalise_parameter(""), Err(RuleError::EmptyParameter));
        assert_eq!(normalise_parameter("utm_source=x"), Err(RuleError::InvalidParameter));
        assert_eq!(normalise_parameter("utm source"), Err(RuleError::InvalidParameter));
        assert_eq!(normalise_domain("example.org/path"), Err(RuleError::InvalidDomain));
        assert_eq!(normalise_domain("exam ple.org"), Err(RuleError::InvalidDomain));
    }

    #[test]
    fn the_family_rule_answers_only_for_names_without_a_rule_of_their_own() {
        let mut rules = Rules::default();
        rules.set_enabled("", CAMPAIGN_FAMILY, false);
        assert!(rules.removes("example.org", "utm_source"));
        assert!(!rules.removes("example.org", "utm_whatever_they_invented"));

        let mut rules = Rules::default();
        rules.set_enabled("", "utm_source", false);
        assert!(!rules.removes("example.org", "utm_source"));
        assert!(rules.removes("example.org", "utm_whatever_they_invented"));
    }

    #[test]
    fn switching_a_name_off_for_a_site_beats_a_global_rule_of_your_own() {
        let mut rules = Rules::default();
        rules.add("", "si").unwrap();
        rules.set_enabled("youtube.com", "si", false);
        assert!(!rules.removes("m.youtube.com", "si"));
        assert!(rules.removes("example.org", "si"));
        assert_eq!(
            rules
                .catalogue()
                .iter()
                .find(|rule| rule.scope == "youtube.com" && rule.parameter == "si")
                .map(|rule| rule.enabled),
            Some(false)
        );
    }

    #[test]
    fn a_rule_already_in_the_list_is_refused_and_your_own_can_be_removed() {
        let mut rules = Rules::default();
        assert_eq!(rules.add("", "fbclid"), Err(RuleError::AlreadyListed));
        assert_eq!(rules.add("YouTube.com", "si"), Err(RuleError::AlreadyListed));
        rules.add("shop.example", "aff").unwrap();
        assert_eq!(rules.add("shop.example", "aff"), Err(RuleError::AlreadyListed));
        rules.set_enabled("shop.example", "aff", false);

        rules.remove("shop.example", "aff");
        assert_eq!(rules, Rules::default());
    }

    #[test]
    fn the_catalogue_opens_with_the_family_and_marks_what_is_yours() {
        let mut rules = Rules::default();
        rules.add("", "ref").unwrap();
        let catalogue = rules.catalogue();
        assert_eq!(catalogue[0].parameter, CAMPAIGN_FAMILY);
        assert!(
            catalogue
                .iter()
                .take_while(|rule| rule.scope.is_empty())
                .any(|rule| rule.parameter == "ref" && rule.custom)
        );
        assert!(catalogue.iter().all(|rule| rule.enabled));
        assert!(catalogue.windows(2).all(|pair| pair[0].scope <= pair[1].scope));
    }

    #[test]
    fn entries_that_do_not_decode_are_dropped_on_the_way_in() {
        let restored =
            Rules::from_entries(&["nonsense".into()], &["*\tfbclid".into(), "broken".into()]);
        assert!(restored.added.is_empty());
        assert_eq!(restored.disabled, vec!["*\tfbclid".to_owned()]);
    }
}
