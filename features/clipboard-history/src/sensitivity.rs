//! Guessing whether a piece of text is a secret.
//!
//! The reliable signal is the mime type a password manager sets, and that is handled where the
//! selection is read. This is the second line: text that nobody marked, but that has the shape
//! of a key. It is a guess, so it errs toward dropping one copy too many.

const NAMES: [&str; 7] =
    ["password", "passwd", "secret", "token", "apikey", "api_key", "authorization"];

const SHORTEST: usize = 20;
const LONGEST: usize = 160;

pub fn looks_sensitive(text: &str) -> bool {
    let lowered = text.to_lowercase();
    if NAMES.iter().any(|name| lowered.contains(name)) {
        return true;
    }
    if is_a_web_address(text) || is_an_identifier(text) {
        return false;
    }

    let length = text.chars().count();
    (SHORTEST..=LONGEST).contains(&length)
        && !text.chars().any(char::is_whitespace)
        && text.chars().any(char::is_alphabetic)
        && text.chars().any(|character| character.is_ascii_digit())
        && text.chars().any(|character| !character.is_alphanumeric())
}

fn is_a_web_address(text: &str) -> bool {
    url::Url::parse(text)
        .map(|address| matches!(address.scheme(), "http" | "https") && address.host().is_some())
        .unwrap_or(false)
}

/// An identifier is nobody's secret, and it has exactly the shape the guess above rejects.
fn is_an_identifier(text: &str) -> bool {
    let trimmed = text.trim_start_matches('{').trim_end_matches('}');
    let groups: Vec<&str> = trimmed.split('-').collect();
    let lengths = [8, 4, 4, 4, 12];

    groups.len() == lengths.len()
        && groups
            .iter()
            .zip(lengths)
            .all(|(group, wanted)| {
                group.len() == wanted && group.chars().all(|c| c.is_ascii_hexdigit())
            })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_secret_is_one_whatever_it_looks_like() {
        assert!(looks_sensitive("my password is hunter2"));
        assert!(looks_sensitive("Authorization: Bearer abc"));
    }

    #[test]
    fn an_ordinary_sentence_is_not() {
        assert!(!looks_sensitive("meet me at the usual place tomorrow"));
        assert!(!looks_sensitive("hello"));
    }

    #[test]
    fn a_web_address_is_not_a_secret() {
        assert!(!looks_sensitive("https://github.com/novasvilla/kavverna?tab=readme"));
    }

    #[test]
    fn an_identifier_is_not_a_secret() {
        assert!(!looks_sensitive("6ba7b810-9dad-11d1-80b4-00c04fd430c8"));
        assert!(!looks_sensitive("{6ba7b810-9dad-11d1-80b4-00c04fd430c8}"));
    }

    #[test]
    fn a_long_unbroken_mixed_string_is_treated_as_a_key() {
        assert!(looks_sensitive("sk-live-4f9a2b71c8e0d3a6f5b2"));
    }

    #[test]
    fn a_long_word_without_digits_or_symbols_is_not() {
        assert!(!looks_sensitive("supercalifragilisticexpialidocious"));
    }
}
