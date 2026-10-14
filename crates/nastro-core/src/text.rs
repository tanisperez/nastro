//! Case and accent folding for sorting and searching.

/// Letters that fold to a plain ASCII sequence: Latin-1 Supplement and Latin Extended-A, in
/// lowercase (the input is lowercased first). Each entry is the letters and what they fold to.
const FOLDS: &[(&str, &str)] = &[
    ("àáâãäåāăą", "a"),
    ("æ", "ae"),
    ("çćĉċč", "c"),
    ("ďđ", "d"),
    ("èéêëēĕėęě", "e"),
    ("ĝğġģ", "g"),
    ("ĥħ", "h"),
    ("ìíîïĩīĭįı", "i"),
    ("ĵ", "j"),
    ("ķ", "k"),
    ("ĺļľŀł", "l"),
    ("ñńņňŉ", "n"),
    ("òóôõöøōŏő", "o"),
    ("œ", "oe"),
    ("ŕŗř", "r"),
    ("śŝşš", "s"),
    ("ß", "ss"),
    ("ţťŧ", "t"),
    ("ùúûüũūŭůűų", "u"),
    ("ŵ", "w"),
    ("ýÿŷ", "y"),
    ("źżž", "z"),
];

/// `text` lowercased with accents removed, so "Dvořák" becomes "dvorak".
pub(crate) fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match FOLDS.iter().find(|(letters, _)| letters.contains(c)) {
            Some((_, plain)) => folded.push_str(plain),
            None => folded.push(c),
        }
    }
    folded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_accents() {
        assert_eq!(fold("Gymnopédie"), "gymnopedie");
        assert_eq!(fold("Dvořák"), "dvorak");
        assert_eq!(fold("ÅNGSTRÖM Łódź"), "angstrom lodz");
        assert_eq!(fold("Straße Œuvre"), "strasse oeuvre");
    }

    #[test]
    fn leaves_other_text_alone() {
        assert_eq!(fold("Track 01 - 愛"), "track 01 - 愛");
    }
}
