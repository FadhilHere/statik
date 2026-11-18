//! Turning arbitrary header text into valid, unique variable names.

const MAX_LEN: usize = 64;

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Make `raw` a valid Statik variable name that `taken` does not report as used.
///
/// Rules (as SPSS does on import): characters other than letters, digits and `_ . $ # @` become `_`;
/// a name that does not start with a letter gets the prefix `V`; names are cut to 64 characters;
/// an empty name becomes `VAR00001`, `VAR00002`, ... (the first one that is free); a duplicate gets the
/// suffix `_1`, `_2`, ...
pub fn sanitize_name(raw: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (1u32..)
            .map(|n| format!("VAR{n:05}"))
            .find(|c| !taken(c))
            .expect("unbounded range");
    }
    let mut base: String = trimmed
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '#' | '@') { c } else { '_' })
        .collect();
    if !base.chars().next().is_some_and(char::is_alphabetic) {
        base.insert(0, 'V');
    }
    let base = truncate_chars(&base, MAX_LEN);
    if !taken(&base) {
        return base;
    }
    for n in 1u32.. {
        let suffix = format!("_{n}");
        let cand = format!("{}{suffix}", truncate_chars(&base, MAX_LEN - suffix.chars().count()));
        if !taken(&cand) {
            return cand;
        }
    }
    unreachable!("unbounded range")
}
