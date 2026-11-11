//! Text decoding with encoding detection (UTF-8/16 BOMs, legacy code pages incl. Thai TIS-620/Windows-874).

use std::sync::OnceLock;

static TLD_HINT: OnceLock<Vec<u8>> = OnceLock::new();

/// Hint the legacy-encoding detector with the user's region (e.g. `b"th"` for Thai),
/// derived from the OS locale. Improves detection of short TIS-620 files.
pub fn set_region_hint(tld: &str) {
    let _ = TLD_HINT.set(tld.to_ascii_lowercase().into_bytes());
}

pub fn decode_text(bytes: &[u8]) -> String {
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(bytes) {
        return enc.decode_without_bom_handling(&bytes[bom_len..]).0.into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    let mut det = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
    det.feed(bytes, true);
    let enc = det.guess(TLD_HINT.get().map(|v| v.as_slice()), chardetng::Utf8Detection::Allow);
    enc.decode_without_bom_handling(bytes).0.into_owned()
}

/// Normalize line endings to `\n`.
pub fn normalize_newlines(s: &str) -> String {
    if !s.contains('\r') {
        return s.to_owned();
    }
    s.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_thai_legacy() {
        let text = "สวัสดีครับ วันนี้อากาศดีมาก เราจะไปเที่ยวทะเลกันที่จังหวัดชลบุรี ";
        let (bytes, _, _) = encoding_rs::WINDOWS_874.encode(text);
        assert_eq!(decode_text(&bytes), text);
    }
    #[test]
    fn decodes_utf8_bom() {
        assert_eq!(decode_text(b"\xEF\xBB\xBFhi"), "hi");
    }
}
