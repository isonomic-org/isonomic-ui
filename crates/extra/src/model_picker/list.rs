// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pure parts of the model list: what a search keeps, how models fall
//! into provider groups, how a context window reads and what a row says.

use super::ModelOption;

/// Whether a model matches what was typed in the search: every word typed
/// must appear, in any case, in its name, its tagline or its provider.
pub(super) fn matches_query(model: &ModelOption, query: &str) -> bool {
    let haystack = [
        Some(model.name.as_ref()),
        model.tagline.as_deref(),
        model.provider.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase();
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

/// The models to show, by position, in the order they are drawn: grouped
/// by provider in the order each provider first appears, and within a
/// group in the order they were added. Models without a provider keep
/// their own place among the groups.
pub(super) fn visible_models(models: &[ModelOption], query: &str) -> Vec<usize> {
    let mut groups: Vec<(Option<&str>, Vec<usize>)> = Vec::new();
    for (ix, model) in models.iter().enumerate() {
        if !matches_query(model, query) {
            continue;
        }
        let key = model.provider.as_deref();
        match groups
            .iter_mut()
            .find(|(group, _)| key.is_some() && *group == key)
        {
            Some((_, members)) => members.push(ix),
            None => groups.push((key, vec![ix])),
        }
    }
    groups
        .into_iter()
        .flat_map(|(_, members)| members)
        .collect()
}

/// Whether a provider heading goes above the model at `at` in the visible
/// list: when it has a provider and the one above it has another.
pub(super) fn starts_group(models: &[ModelOption], visible: &[usize], at: usize) -> bool {
    let Some(provider) = visible.get(at).and_then(|ix| models[*ix].provider.as_ref()) else {
        return false;
    };
    match at.checked_sub(1).and_then(|above| visible.get(above)) {
        Some(above) => models[*above].provider.as_ref() != Some(provider),
        None => true,
    }
}

/// The model an arrow key moves to, by position in the full list, moving
/// through the visible ones only. It stops at the ends.
pub(super) fn step_visible(active: Option<usize>, visible: &[usize], key: &str) -> Option<usize> {
    let at = active.and_then(|ix| visible.iter().position(|shown| *shown == ix));
    let next = super::step_active(at, visible.len(), key)?;
    visible.get(next).copied()
}

/// A context window in short words: "8K", "128K", "1M", "1.5M".
pub fn context_words(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        let millions = tokens as f64 / 1_000_000.;
        if (millions - millions.round()).abs() < 0.05 {
            format!("{}M", millions.round() as u64)
        } else {
            format!("{millions:.1}M")
        }
    } else if tokens >= 1_000 {
        format!("{}K", (tokens as f64 / 1_000.).round() as u64)
    } else {
        tokens.to_string()
    }
}

/// How full a context bar is against the largest window on offer. A square
/// root keeps a small window visible beside a very large one.
pub(super) fn context_fraction(tokens: u64, largest: u64) -> f32 {
    if largest == 0 {
        return 0.;
    }
    ((tokens.min(largest) as f64 / largest as f64).sqrt() as f32).clamp(0., 1.)
}

/// What a screen reader hears for a model's row: its name, provider and
/// tagline, then each rating and the context window in words.
pub(super) fn row_words(model: &ModelOption, columns: &[impl AsRef<str>]) -> String {
    let mut words = vec![model.name.to_string()];
    words.extend(
        model
            .provider
            .as_ref()
            .map(|provider| format!("from {provider}")),
    );
    words.extend(model.tagline.as_ref().map(ToString::to_string));
    for (column, rating) in columns.iter().zip(&model.ratings) {
        words.push(format!(
            "{} {} of {}",
            column.as_ref(),
            super::lit_steps(*rating),
            super::STEPS
        ));
    }
    words.extend(
        model
            .context
            .map(|tokens| format!("context {} tokens", context_words(tokens))),
    );
    words.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn models() -> Vec<ModelOption> {
        vec![
            ModelOption::new("a", "Atlas")
                .provider("North")
                .tagline("Audits"),
            ModelOption::new("b", "Brook").provider("South"),
            ModelOption::new("c", "Cove")
                .provider("North")
                .tagline("Quick lookups"),
            ModelOption::new("d", "Router"),
        ]
    }

    #[test]
    fn search_matches_every_word_anywhere() {
        let models = models();
        assert!(matches_query(&models[0], ""));
        assert!(matches_query(&models[0], "atl"));
        assert!(matches_query(&models[0], "NORTH audits"));
        assert!(!matches_query(&models[0], "north quick"));
        assert!(matches_query(&models[2], "north quick"));
    }

    #[test]
    fn models_group_by_provider_in_first_seen_order() {
        let models = models();
        assert_eq!(visible_models(&models, ""), [0, 2, 1, 3]);
        assert_eq!(visible_models(&models, "north"), [0, 2]);
        assert_eq!(visible_models(&models, "zzz"), Vec::<usize>::new());
        let visible = visible_models(&models, "");
        let heads: Vec<_> = (0..visible.len())
            .map(|at| starts_group(&models, &visible, at))
            .collect();
        assert_eq!(heads, [true, false, true, false]);
    }

    #[test]
    fn arrows_move_through_the_visible_models_only() {
        let visible = [0, 2, 1];
        assert_eq!(step_visible(None, &visible, "down"), Some(0));
        assert_eq!(step_visible(Some(0), &visible, "down"), Some(2));
        assert_eq!(step_visible(Some(2), &visible, "down"), Some(1));
        assert_eq!(step_visible(Some(1), &visible, "down"), Some(1));
        // A model the search hid starts again from the top.
        assert_eq!(step_visible(Some(3), &visible, "down"), Some(0));
        assert_eq!(step_visible(Some(2), &visible, "end"), Some(1));
        assert_eq!(step_visible(None, &[], "down"), None);
    }

    #[test]
    fn context_windows_read_in_short_words() {
        assert_eq!(context_words(512), "512");
        assert_eq!(context_words(8_192), "8K");
        assert_eq!(context_words(128_000), "128K");
        assert_eq!(context_words(1_000_000), "1M");
        assert_eq!(context_words(1_500_000), "1.5M");
    }

    #[test]
    fn context_bars_keep_small_windows_visible() {
        assert_eq!(context_fraction(0, 0), 0.);
        assert_eq!(context_fraction(1_000_000, 1_000_000), 1.);
        assert!((context_fraction(250_000, 1_000_000) - 0.5).abs() < 1e-6);
        assert_eq!(context_fraction(2_000_000, 1_000_000), 1.);
    }

    #[test]
    fn a_row_says_its_ratings_and_context_in_words() {
        let model = ModelOption::new("a", "Atlas")
            .provider("North")
            .tagline("Audits")
            .ratings([2, 9])
            .context_window(200_000);
        assert_eq!(
            row_words(&model, &["Speed", "Price"]),
            "Atlas, from North, Audits, Speed 2 of 5, Price 5 of 5, context 200K tokens"
        );
        assert_eq!(
            row_words(&ModelOption::new("x", "Plain"), &[""; 0]),
            "Plain"
        );
    }
}
