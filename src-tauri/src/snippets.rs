//! Snippets: say a trigger phrase, get a saved block of text (an address, a link, a signature).

use crate::store::Snippet;

/// Lowercase words of `text`, punctuation dropped: "My e-mail, please." -> ["my", "e", "mail", "please"].
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '\u{2019}')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// The expansion when the whole dictation is just a trigger phrase (ignoring case and
/// punctuation, so "My calendar link." matches "my calendar link").
pub fn whole_match<'a>(text: &str, snippets: &'a [Snippet]) -> Option<&'a str> {
    let said = words(text);
    if said.is_empty() {
        return None;
    }
    snippets
        .iter()
        .find(|s| words(&s.trigger) == said)
        .map(|s| s.expansion.as_str())
}

/// Replaces trigger phrases inside `text` with their expansions. Matching ignores case and the
/// punctuation between words; a match must start and end on word boundaries. Longer triggers
/// win over shorter ones that overlap them.
pub fn expand(text: &str, snippets: &[Snippet]) -> String {
    let mut triggers: Vec<(Vec<String>, &str)> = snippets
        .iter()
        .map(|s| (words(&s.trigger), s.expansion.as_str()))
        .filter(|(w, _)| !w.is_empty())
        .collect();
    if triggers.is_empty() {
        return text.to_string();
    }
    triggers.sort_by_key(|(w, _)| std::cmp::Reverse(w.len()));

    // Word spans (byte ranges) of the text, with the same word rule as `words`.
    let mut spans = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        let in_word = c.is_alphanumeric() || c == '\'' || c == '\u{2019}';
        match (in_word, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                spans.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        spans.push((s, text.len()));
    }
    let lower: Vec<String> = spans
        .iter()
        .map(|&(a, b)| text[a..b].to_lowercase())
        .collect();

    let mut out = String::with_capacity(text.len());
    let mut copied = 0; // byte offset of text already copied to `out`
    let mut i = 0;
    while i < spans.len() {
        let hit = triggers
            .iter()
            .find(|(w, _)| i + w.len() <= lower.len() && lower[i..i + w.len()] == w[..]);
        match hit {
            Some((w, expansion)) => {
                let (from, _) = spans[i];
                let (_, to) = spans[i + w.len() - 1];
                out.push_str(&text[copied..from]);
                out.push_str(expansion);
                copied = to;
                i += w.len();
            }
            None => i += 1,
        }
    }
    out.push_str(&text[copied..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(trigger: &str, expansion: &str) -> Snippet {
        Snippet {
            id: 0,
            trigger: trigger.into(),
            expansion: expansion.into(),
        }
    }

    #[test]
    fn whole_dictation_matches_ignoring_case_and_punctuation() {
        let list = [s("my calendar link", "https://cal.example/me")];
        assert_eq!(
            whole_match("My calendar link.", &list),
            Some("https://cal.example/me")
        );
        assert_eq!(whole_match("my calendar link please", &list), None);
        assert_eq!(whole_match("", &list), None);
    }

    #[test]
    fn expands_a_trigger_inside_a_sentence() {
        let list = [s("my address", "1 Main St\nSpringfield")];
        assert_eq!(
            expand("Please ship it to my address, thanks.", &list),
            "Please ship it to 1 Main St\nSpringfield, thanks."
        );
    }

    #[test]
    fn matches_whole_words_only() {
        let list = [s("sig", "Best, Sam")];
        assert_eq!(expand("signal sig.", &list), "signal Best, Sam.");
    }

    #[test]
    fn longer_trigger_wins() {
        let list = [s("my email", "a@x.io"), s("my email signature", "-- A")];
        assert_eq!(expand("add my email signature", &list), "add -- A");
        assert_eq!(expand("add My Email now", &list), "add a@x.io now");
    }

    #[test]
    fn punctuation_between_trigger_words_is_ignored() {
        let list = [s("zoom link", "https://zoom.example/j/1")];
        assert_eq!(
            expand("Join: Zoom, link.", &list),
            "Join: https://zoom.example/j/1."
        );
    }

    #[test]
    fn unicode_does_not_panic() {
        let list = [s("café menu", "☕ list")];
        assert_eq!(expand("Le café menu 🎉", &list), "Le ☕ list 🎉");
        assert_eq!(expand("日本語", &list), "日本語");
    }
}
