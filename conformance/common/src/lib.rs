//! Helpers shared by the conformance runner and the reference mock provider.
//!
//! The runner expands `${PROJECT_URI}` in fixtures and the mock provider
//! resolves `entry` URIs back to paths; both sides must agree on the file-URI
//! codec byte for byte.

use std::path::{Path, PathBuf};

/// `file:` URI for an absolute path, percent-encoding bytes outside the
/// unreserved set. Returns `None` when the path is not valid UTF-8.
pub fn path_to_file_uri(path: &Path) -> Option<String> {
    let path = path.to_str()?;
    let mut uri = String::from("file://");
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                uri.push(byte as char)
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    Some(uri)
}

/// Absolute path for a `file:///` URI, percent-decoding the path component.
/// Returns `None` when the URI is not an absolute file URI or carries invalid
/// percent-encoding or UTF-8.
pub fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let path = uri.strip_prefix("file:///")?;
    Some(PathBuf::from(format!("/{}", percent_decode(path)?)))
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let high = hex_value(*bytes.get(i + 1)?)?;
                let low = hex_value(*bytes.get(i + 2)?)?;
                decoded.push(high * 16 + low);
                i += 3;
            }
            byte => {
                decoded.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uri_round_trip() {
        let path = Path::new("/tmp/some project/entry.xdl");
        let uri = path_to_file_uri(path).expect("UTF-8 path");
        assert_eq!(uri, "file:///tmp/some%20project/entry.xdl");
        assert_eq!(file_uri_to_path(&uri), Some(path.to_path_buf()));
    }

    #[test]
    fn rejects_non_file_uris_and_bad_escapes() {
        assert_eq!(file_uri_to_path("https://x/y"), None);
        assert_eq!(file_uri_to_path("file://relative"), None);
        assert_eq!(file_uri_to_path("file:///bad%zz"), None);
        assert_eq!(file_uri_to_path("file:///bad%2"), None);
    }
}
