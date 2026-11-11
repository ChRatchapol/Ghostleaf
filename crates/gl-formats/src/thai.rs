//! Thai has no spaces between words. Line breaking needs word boundaries, so we
//! insert ZERO WIDTH SPACE (U+200B) at dictionary/LSTM word boundaries
//! (ICU4X segmenter, fully offline) before layout.

use icu_segmenter::WordSegmenter;
use icu_segmenter::options::WordBreakInvariantOptions;

pub const ZWSP: char = '\u{200B}';

pub fn is_thai(c: char) -> bool {
    ('\u{0E00}'..='\u{0E7F}').contains(&c)
}

pub fn contains_thai(s: &str) -> bool {
    s.chars().any(is_thai)
}

/// Insert ZWSP between Thai words. Text without Thai is returned unchanged.
pub fn insert_word_breaks(s: &str) -> String {
    if !contains_thai(s) {
        return s.to_owned();
    }
    let seg = WordSegmenter::new_auto(WordBreakInvariantOptions::default());
    let mut out = String::with_capacity(s.len() + s.len() / 8);
    let mut last = 0;
    for b in seg.segment_str(s) {
        if b == 0 || b >= s.len() {
            continue;
        }
        let prev = s[..b].chars().next_back();
        let next = s[b..].chars().next();
        out.push_str(&s[last..b]);
        if let (Some(p), Some(n)) = (prev, next)
            && (is_thai(p) || is_thai(n))
            && !p.is_whitespace()
            && !n.is_whitespace()
            && p != ZWSP
            && n != ZWSP
        {
            out.push(ZWSP);
        }
        last = b;
    }
    out.push_str(&s[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segments_thai() {
        let s = "ภาษาไทยไม่มีช่องว่าง";
        let out = insert_word_breaks(s);
        assert!(out.contains(ZWSP), "{out}");
        assert_eq!(out.replace(ZWSP, ""), s);
    }
    #[test]
    fn leaves_latin_alone() {
        assert_eq!(insert_word_breaks("hello world"), "hello world");
    }
}
