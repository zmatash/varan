//! Load and save source files byte-for-byte faithfully: line endings, a
//! UTF-8 byte-order mark and non-UTF-8 (Latin-1 / Windows-1252) text are
//! remembered on load and reproduced on save. Tabs are never touched.

/// How a source file was stored on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct FileFormat {
    pub crlf: bool,
    pub bom: bool,
    /// The file was not valid UTF-8, so each byte was read as one character.
    pub latin1: bool,
}

impl Default for FileFormat {
    fn default() -> Self {
        // New files use Windows line endings.
        FileFormat { crlf: true, bom: false, latin1: false }
    }
}

pub fn decode_source(bytes: &[u8]) -> (String, FileFormat) {
    let (bom, body) = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (true, rest),
        None => (false, bytes),
    };
    let (text, latin1) = match std::str::from_utf8(body) {
        Ok(s) => (s.to_string(), false),
        Err(_) => (body.iter().map(|&b| b as char).collect::<String>(), true),
    };
    let crlf = text.contains("\r\n") || !text.contains('\n');
    (text.replace("\r\n", "\n"), FileFormat { crlf, bom, latin1 })
}

pub fn encode_source(text: &str, format: &FileFormat) -> Vec<u8> {
    let text = if format.crlf { text.replace('\n', "\r\n") } else { text.to_string() };
    let mut out = Vec::new();
    if format.bom {
        out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    if format.latin1 && text.chars().all(|c| (c as u32) < 256) {
        out.extend(text.chars().map(|c| c as u8));
    } else {
        out.extend_from_slice(text.as_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_round_trip_unchanged() {
        let cases: [&[u8]; 5] = [
            b"start\tMOV\tR0, #1\t; tabbed\r\n\tSWI\t2\r\n",
            b"start\tMOV\tR0, #1\n        SWI 2\n",
            b"\xEF\xBB\xBFlabel\tDEFB\t\"x\"\r\n",
            b"; cost \xA35 (latin-1)\n\tSWI 2\n",
            b"no newline at end",
        ];
        for bytes in cases {
            let (text, fmt) = decode_source(bytes);
            assert!(!text.contains('\r'));
            assert!(!text.starts_with('\u{FEFF}'));
            assert_eq!(encode_source(&text, &fmt), bytes, "{:?}", String::from_utf8_lossy(bytes));
        }
    }
}
