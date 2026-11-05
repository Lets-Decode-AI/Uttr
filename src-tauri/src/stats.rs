//! Usage statistics for the Home dashboard, computed from the transcript history in the
//! user's local days.

use serde::Serialize;

/// Typing speed used to estimate the time dictation saved.
pub const TYPING_WPM: f64 = 40.0;
/// Days shown in the words-per-day chart.
pub const CHART_DAYS: i64 = 30;
const DAY_MS: i64 = 86_400_000;

/// One transcript, as the statistics need it.
#[derive(Debug, Clone)]
pub struct Row {
    pub created_at: i64,
    pub words: i64,
    pub duration_ms: i64,
    pub app: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayWords {
    /// Local date, `YYYY-MM-DD`.
    pub date: String,
    pub words: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppWords {
    pub app: String,
    pub words: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub total_words: i64,
    pub dictations: i64,
    pub speaking_ms: i64,
    /// Words per minute over all speaking time.
    pub avg_wpm: f64,
    pub current_streak: u32,
    pub longest_streak: u32,
    /// Minutes saved against typing at [`TYPING_WPM`], never negative.
    pub time_saved_min: f64,
    /// The last [`CHART_DAYS`] days, oldest first, including days without dictation.
    pub days: Vec<DayWords>,
    /// Up to five apps with the most dictated words.
    pub top_apps: Vec<AppWords>,
}

/// `offset_min` is JavaScript's `getTimezoneOffset()`: minutes to add to local time for UTC.
pub fn compute(rows: &[Row], now_ms: i64, offset_min: i32) -> Stats {
    let local_day = |ms: i64| (ms - i64::from(offset_min) * 60_000).div_euclid(DAY_MS);
    let today = local_day(now_ms);

    let total_words: i64 = rows.iter().map(|r| r.words).sum();
    let speaking_ms: i64 = rows.iter().map(|r| r.duration_ms.max(0)).sum();
    let speaking_min = speaking_ms as f64 / 60_000.0;
    let avg_wpm = if speaking_min > 0.0 {
        total_words as f64 / speaking_min
    } else {
        0.0
    };
    let time_saved_min = (total_words as f64 / TYPING_WPM - speaking_min).max(0.0);

    let mut words_by_day = std::collections::BTreeMap::<i64, i64>::new();
    let mut words_by_app = std::collections::HashMap::<&str, i64>::new();
    for r in rows {
        *words_by_day.entry(local_day(r.created_at)).or_default() += r.words;
        if let Some(app) = r.app.as_deref().filter(|a| !a.is_empty()) {
            *words_by_app.entry(app).or_default() += r.words;
        }
    }

    // Streaks over the sorted distinct days.
    let (mut longest, mut run, mut prev) = (0u32, 0u32, None::<i64>);
    for &day in words_by_day.keys() {
        run = if prev == Some(day - 1) { run + 1 } else { 1 };
        longest = longest.max(run);
        prev = Some(day);
    }
    let current_streak = match prev {
        Some(last) if last >= today - 1 => run,
        _ => 0,
    };

    let days = (today - CHART_DAYS + 1..=today)
        .map(|d| DayWords {
            date: date_of(d),
            words: words_by_day.get(&d).copied().unwrap_or(0),
        })
        .collect();
    let mut top_apps: Vec<AppWords> = words_by_app
        .into_iter()
        .map(|(app, words)| AppWords {
            app: app.to_string(),
            words,
        })
        .collect();
    top_apps.sort_by(|a, b| b.words.cmp(&a.words).then_with(|| a.app.cmp(&b.app)));
    top_apps.truncate(5);

    Stats {
        total_words,
        dictations: rows.len() as i64,
        speaking_ms,
        avg_wpm,
        current_streak,
        longest_streak: longest,
        time_saved_min,
        days,
        top_apps,
    }
}

/// `YYYY-MM-DD` for a count of days since 1970-01-01.
pub fn date_of(day: i64) -> String {
    // Howard Hinnant's civil_from_days.
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Transcripts created before this are deleted under a "keep history for `days` days" rule.
pub fn retention_cutoff(now_ms: i64, days: Option<u32>) -> Option<i64> {
    days.map(|d| now_ms - i64::from(d) * DAY_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3_600_000;
    /// 2026-11-05 12:00 UTC.
    const NOW: i64 = 1_793_880_000_000;

    fn row(ms: i64, words: i64, dur_ms: i64, app: &str) -> Row {
        Row {
            created_at: ms,
            words,
            duration_ms: dur_ms,
            app: Some(app.into()),
        }
    }

    #[test]
    fn retention_cutoff_is_that_many_days_back() {
        assert_eq!(retention_cutoff(NOW, None), None);
        assert_eq!(retention_cutoff(NOW, Some(7)), Some(NOW - 7 * DAY_MS));
    }

    #[test]
    fn dates_from_day_numbers() {
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(date_of(NOW / DAY_MS), "2026-11-05");
        assert_eq!(date_of(11_016), "2000-02-29");
        assert_eq!(date_of(-1), "1969-12-31");
    }

    #[test]
    fn empty_history_is_all_zero_with_thirty_days() {
        let s = compute(&[], NOW, 0);
        assert_eq!(
            (
                s.total_words,
                s.dictations,
                s.current_streak,
                s.longest_streak
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(s.avg_wpm, 0.0);
        assert_eq!(s.time_saved_min, 0.0);
        assert_eq!(s.days.len(), 30);
        assert_eq!(s.days.last().unwrap().date, "2026-11-05");
        assert_eq!(s.days[0].date, "2026-10-07");
        assert!(s.days.iter().all(|d| d.words == 0));
        assert!(s.top_apps.is_empty());
    }

    #[test]
    fn totals_speed_and_time_saved() {
        // 300 words in 2 minutes of speech: 150 WPM; typing them takes 7.5 min, so 5.5 saved.
        let rows = [
            row(NOW - H, 100, 40_000, "Mail"),
            row(NOW - 2 * H, 200, 80_000, "Slack"),
        ];
        let s = compute(&rows, NOW, 0);
        assert_eq!(
            (s.total_words, s.dictations, s.speaking_ms),
            (300, 2, 120_000)
        );
        assert!((s.avg_wpm - 150.0).abs() < 1e-9);
        assert!((s.time_saved_min - 5.5).abs() < 1e-9);
    }

    #[test]
    fn time_saved_never_goes_negative() {
        let s = compute(&[row(NOW, 2, 600_000, "Notes")], NOW, 0);
        assert_eq!(s.time_saved_min, 0.0);
    }

    #[test]
    fn streaks_count_consecutive_days_ending_today_or_yesterday() {
        let d = |n: i64| NOW - n * 24 * H;
        // Today, yesterday, two days ago; then a gap; then four days in a row.
        let mut rows: Vec<Row> = [0, 1, 2].iter().map(|&n| row(d(n), 1, 1, "a")).collect();
        rows.extend([5, 6, 7, 8].iter().map(|&n| row(d(n), 1, 1, "a")));
        let s = compute(&rows, NOW, 0);
        assert_eq!((s.current_streak, s.longest_streak), (3, 4));
        // Nothing yet today: a streak through yesterday still counts.
        let s = compute(&rows[1..], NOW, 0);
        assert_eq!(s.current_streak, 2);
        // Last dictation two days ago: the streak is broken.
        let s = compute(&rows[2..], NOW, 0);
        assert_eq!(s.current_streak, 0);
    }

    #[test]
    fn days_are_local_days() {
        // 2026-11-05 02:00 UTC is still 2026-11-04 in New York (UTC-5, offset +300).
        let early = NOW - 10 * H;
        let utc = compute(&[row(early, 7, 1, "a")], NOW, 0);
        assert_eq!(utc.days.last().unwrap().words, 7);
        let ny = compute(&[row(early, 7, 1, "a")], NOW, 300);
        assert_eq!(ny.days.last().unwrap().words, 0);
        assert_eq!(ny.days[28].words, 7);
        assert_eq!(ny.days[28].date, "2026-11-04");
    }

    #[test]
    fn top_apps_by_words_at_most_five() {
        let rows: Vec<Row> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .enumerate()
            .map(|(i, app)| row(NOW, (i as i64 + 1) * 10, 1, app))
            .chain([Row {
                created_at: NOW,
                words: 999,
                duration_ms: 1,
                app: None,
            }])
            .collect();
        let s = compute(&rows, NOW, 0);
        let apps: Vec<&str> = s.top_apps.iter().map(|a| a.app.as_str()).collect();
        assert_eq!(apps, ["f", "e", "d", "c", "b"]);
        assert_eq!(s.top_apps[0].words, 60);
    }

    #[test]
    fn old_dictations_count_in_totals_but_not_the_chart() {
        let s = compute(&[row(NOW - 90 * 24 * H, 50, 1, "a")], NOW, 0);
        assert_eq!(s.total_words, 50);
        assert_eq!(s.days.iter().map(|d| d.words).sum::<i64>(), 0);
    }
}
