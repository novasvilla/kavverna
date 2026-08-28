//! Guessing whether text is a secret nobody marked as one. The reliable signal is the mime
//! type, handled in `selection`. This is a guess, and it errs toward dropping one copy too many.

const NAMES: [&str; 7] =
    ["password", "passwd", "secret", "token", "apikey", "api_key", "authorization"];

const SHORTEST: usize = 20;
const LONGEST: usize = 160;

pub fn looks_sensitive(text: &str) -> bool {
    let lowered = text.to_lowercase();
    if NAMES.iter().any(|name| lowered.contains(name)) {
        return true;
    }
    // Three shapes the length and whitespace test below cannot see: a token longer than it
    // allows, a key block that is nothing but whitespace and line breaks, and a password sitting
    // inside a URL, which the web address test would otherwise wave through.
    if is_a_web_token(text) || is_an_armoured_block(text) || carries_credentials(text) {
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

/// A JSON web token: three base64url parts separated by dots, the first of which decodes to a
/// header naming an algorithm. Checking the shape rather than the length is what matters, since
/// these run well past the ceiling the general guess works within.
fn is_a_web_token(text: &str) -> bool {
    let text = text.trim();
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return false;
    }
    if !parts
        .iter()
        .all(|part| part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    {
        return false;
    }

    // Every JWT header is a JSON object, and base64url of `{"` begins with these two characters
    // whatever follows, so this needs no decoder.
    parts[0].starts_with("eyJ")
}

/// A PEM block. It is mostly line breaks, and the whitespace test below rejects anything with
/// them, so a private key would otherwise be stored in full.
fn is_an_armoured_block(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line.starts_with("-----BEGIN") && line.ends_with("-----")
    })
}

/// A password inside a URL. `is_a_web_address` deliberately passes ordinary links, and a
/// connection string is a link with a secret in the middle of it.
fn carries_credentials(text: &str) -> bool {
    url::Url::parse(text.trim())
        .map(|address| !address.username().is_empty() || address.password().is_some())
        .unwrap_or(false)
}

fn is_a_web_address(text: &str) -> bool {
    url::Url::parse(text.trim()).map(|address| address.host().is_some()).unwrap_or(false)
}

/// An identifier has the shape the guess below rejects, and is nobody's secret.
fn is_an_identifier(text: &str) -> bool {
    let trimmed = text.trim_start_matches('{').trim_end_matches('}');
    let groups: Vec<&str> = trimmed.split('-').collect();
    let lengths = [8, 4, 4, 4, 12];

    groups.len() == lengths.len()
        && groups.iter().zip(lengths).all(|(group, wanted)| {
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

    /// Well past the length the general guess works within, which is how one used to get through.
    #[test]
    fn a_json_web_token_is_a_secret_however_long_it_runs() {
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
                     eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkphbmUgRG9lIiwiYWRtaW4iOnRydWUsInNjb3Bl\
                     IjoicmVhZCB3cml0ZSBkZWxldGUiLCJpc3MiOiJodHRwczovL2F1dGguZXhhbXBsZS5vcmciLCJp\
                     YXQiOjE1MTYyMzkwMjIsImV4cCI6MTUxNjI0MjYyMn0.\
                     SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

        assert!(looks_sensitive(token));
        assert!(token.chars().count() > LONGEST, "the test stopped covering what it was for");
    }

    #[test]
    fn three_dotted_words_are_not_a_token() {
        assert!(!looks_sensitive("first.second.third"));
        assert!(!looks_sensitive("www.example.com"));
    }

    /// Nothing but line breaks and base64, and the whitespace test rejects anything with a
    /// line break in it, so this used to be stored in full.
    #[test]
    fn a_key_block_is_a_secret_despite_its_line_breaks() {
        let key = "-----BEGIN OPENSSH PRIVATE KEY-----\n\
                   b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtz\n\
                   -----END OPENSSH PRIVATE KEY-----";

        assert!(looks_sensitive(key));
    }

    #[test]
    fn a_link_carrying_a_password_is_a_secret_where_a_plain_one_is_not() {
        assert!(looks_sensitive("postgres://admin:hunter2@db.internal:5432/orders"));
        assert!(looks_sensitive("https://someone:letmein@example.org/private"));
        assert!(!looks_sensitive("postgres://db.internal:5432/orders"));
    }
}
