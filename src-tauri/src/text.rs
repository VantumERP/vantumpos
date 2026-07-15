/// Folds Serbian Latin text for accent- and case-insensitive search:
/// lowercases (Unicode) then maps š→s, ž→z, č→c, ć→c, đ→dj.
pub fn fold_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.to_lowercase().chars() {
        match ch {
            'š' => out.push('s'),
            'ž' => out.push('z'),
            'č' | 'ć' => out.push('c'),
            'đ' => out.push_str("dj"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fold_text;

    #[test]
    fn folds_case_and_serbian_diacritics() {
        assert_eq!(fold_text("KOŠULJA"), "kosulja");
        assert_eq!(fold_text("Žabac"), "zabac");
        assert_eq!(fold_text("Đorđe"), "djordje");
        assert_eq!(fold_text("Čačak-Ćuprija"), "cacak-cuprija");
    }

    #[test]
    fn leaves_plain_ascii_and_digits_unchanged() {
        assert_eq!(fold_text("ABC 123"), "abc 123");
        assert_eq!(fold_text("8600000000010"), "8600000000010");
    }
}
