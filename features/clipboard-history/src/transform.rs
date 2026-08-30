//! Turning what is on the clipboard into something else. Pure text to text, so all of it is
//! tested without a compositor; reading the selection and offering the result back belong to
//! the caller.

/// What can be asked of the current selection. Plain carries no function of its own: making a
/// copy plain is offering only its text again, and the offering is the whole of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transformation {
    Plain,
    Json,
    Markdown,
}

/// Pretty printed when the text is JSON, and said plainly when it is not, since quietly doing
/// nothing looks exactly like a dead button.
pub fn pretty_json(text: &str) -> Result<String, String> {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => serde_json::to_string_pretty(&value)
            .map_err(|err| format!("could not lay the JSON out: {err}")),
        Err(err) => Err(format!("not JSON: {err}")),
    }
}

/// From the `text/html` a rich copy offers, read on demand rather than stored, so the history
/// keeps holding plain text only and nothing richer sits on disk.
pub fn markdown_from_html(html: &str) -> String {
    htmd::convert(html).unwrap_or_else(|_| html.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_comes_back_laid_out() {
        assert_eq!(pretty_json("{\"a\":1}").unwrap(), "{\n  \"a\": 1\n}");
    }

    #[test]
    fn what_is_not_json_says_so_instead_of_failing_quietly() {
        let refusal = pretty_json("not json at all").unwrap_err();
        assert!(refusal.starts_with("not JSON"), "{refusal}");
    }

    /// Numbers and strings are valid JSON on their own, and pretty printing one is a no-op
    /// worth allowing rather than refusing: the button did what was asked.
    #[test]
    fn a_bare_value_is_json_too() {
        assert_eq!(pretty_json("42").unwrap(), "42");
    }

    #[test]
    fn markup_becomes_markdown() {
        assert_eq!(markdown_from_html("<b>bold</b> and <i>italic</i>"), "**bold** and *italic*");
    }

    #[test]
    fn links_keep_their_destination() {
        assert_eq!(
            markdown_from_html("<a href=\"https://example.org\">there</a>"),
            "[there](https://example.org)"
        );
    }
}
