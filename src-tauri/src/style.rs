//! Writing styles: how formal the inserted text should read, chosen per app.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Style {
    /// Full sentences with capitals and punctuation.
    #[default]
    Formal,
    /// Capitals, but no period after a single sentence.
    Casual,
    /// Lowercase start and no period after a single sentence, like a chat message.
    VeryCasual,
}

/// A style rule for one app, matched against the frontmost app's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStyle {
    pub app: String,
    pub style: Style,
}

/// The style for `app`: the first rule whose name matches (ignoring case, and also when one
/// name contains the other, so "Slack" matches "Slack.exe"), else `default`.
pub fn style_for(app: Option<&str>, rules: &[AppStyle], default: Style) -> Style {
    let Some(app) = app
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty())
    else {
        return default;
    };
    let exact = rules
        .iter()
        .find(|r| r.app.trim().to_lowercase() == app)
        .map(|r| r.style);
    exact
        .or_else(|| {
            rules
                .iter()
                .find(|r| {
                    let name = r.app.trim().to_lowercase();
                    !name.is_empty() && (app.contains(&name) || name.contains(&app))
                })
                .map(|r| r.style)
        })
        .unwrap_or(default)
}

impl Style {
    /// Instruction for the AI cleanup prompt.
    pub fn prompt(self) -> &'static str {
        match self {
            Style::Formal => {
                "Write in a clear, professional tone with complete sentences, correct capitalization and full punctuation."
            }
            Style::Casual => {
                "Write in a relaxed, friendly tone. Keep capitalization, but leave off the period after a single sentence."
            }
            Style::VeryCasual => {
                "Write like a quick chat message: start in lowercase, keep punctuation light and leave off the final period."
            }
        }
    }
}

/// Text is one sentence when its only sentence-ending mark is the last character.
fn single_sentence(text: &str) -> bool {
    let body = text.trim_end_matches(['.', '!', '?']);
    !body.contains(['.', '!', '?', '\n'])
}

/// Drops the period after a single sentence (casual styles). `?` and `!` stay.
pub fn trim_final_period(text: &str, style: Style) -> String {
    if style != Style::Formal
        && single_sentence(text)
        && text.ends_with('.')
        && !text.ends_with("..")
    {
        text[..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(app: &str, style: Style) -> AppStyle {
        AppStyle {
            app: app.into(),
            style,
        }
    }

    #[test]
    fn picks_the_matching_app_rule() {
        let rules = [
            rule("Slack", Style::VeryCasual),
            rule("Mail", Style::Formal),
        ];
        assert_eq!(
            style_for(Some("Slack"), &rules, Style::Casual),
            Style::VeryCasual
        );
        assert_eq!(
            style_for(Some("slack.exe"), &rules, Style::Casual),
            Style::VeryCasual
        );
        assert_eq!(
            style_for(Some("MAIL"), &rules, Style::Casual),
            Style::Formal
        );
        assert_eq!(
            style_for(Some("Notes"), &rules, Style::Casual),
            Style::Casual
        );
        assert_eq!(style_for(None, &rules, Style::Casual), Style::Casual);
    }

    #[test]
    fn exact_match_beats_a_partial_one() {
        let rules = [
            rule("Code", Style::Casual),
            rule("Visual Studio Code", Style::Formal),
        ];
        assert_eq!(
            style_for(Some("Visual Studio Code"), &rules, Style::VeryCasual),
            Style::Formal
        );
    }

    #[test]
    fn blank_rules_never_match() {
        let rules = [rule("  ", Style::VeryCasual)];
        assert_eq!(
            style_for(Some("Notes"), &rules, Style::Formal),
            Style::Formal
        );
    }

    #[test]
    fn casual_styles_drop_the_period_of_one_sentence() {
        assert_eq!(
            trim_final_period("See you soon.", Style::Casual),
            "See you soon"
        );
        assert_eq!(
            trim_final_period("See you soon.", Style::Formal),
            "See you soon."
        );
        assert_eq!(trim_final_period("Really?", Style::VeryCasual), "Really?");
        assert_eq!(trim_final_period("Wait...", Style::Casual), "Wait...");
        assert_eq!(
            trim_final_period("One. Two.", Style::Casual),
            "One. Two.",
            "several sentences keep their periods"
        );
    }

    #[test]
    fn styles_serialize_camel_case() {
        assert_eq!(
            serde_json::to_string(&Style::VeryCasual).unwrap(),
            r#""veryCasual""#
        );
    }
}
