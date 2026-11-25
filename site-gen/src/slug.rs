/// Replicates Quartz 4.5.2's exact URL slug rules from `quartz/util/path.ts`,
/// `github-slugger` anchor slugging, and RFC 3986 path percent-encoding.

pub fn sluggify_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for ch in segment.chars() {
        if ch.is_whitespace() {
            out.push('-');
        } else if ch == '&' {
            out.push_str("-and-");
        } else if ch == '%' {
            out.push_str("-percent");
        } else if ch == '?' || ch == '#' {
            // stripped
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn sluggify_path(path: &str) -> String {
    let joined = path
        .split('/')
        .map(sluggify_segment)
        .collect::<Vec<_>>()
        .join("/");
    joined.trim_end_matches('/').to_string()
}

pub fn slugify_file_path(fp: &str) -> String {
    let trimmed = fp.trim_matches('/');
    let (without_ext, ext) = match trimmed.rfind('.') {
        Some(idx) if idx > trimmed.rfind('/').unwrap_or(0) => {
            (&trimmed[..idx], Some(&trimmed[idx..]))
        }
        _ => (trimmed, None),
    };
    let keep_ext = match ext {
        Some(".md") | Some(".html") | None => "",
        Some(other) => other,
    };
    let mut slug = sluggify_path(without_ext);
    if slug == "_index" {
        slug = "index".to_string();
    } else if let Some(prefix) = slug.strip_suffix("/_index") {
        slug = format!("{}/index", prefix);
    }
    format!("{}{}", slug, keep_ext)
}

pub fn slug_anchor(heading: &str) -> String {
    let mut out = String::with_capacity(heading.len());
    for ch in heading.chars() {
        for lower in ch.to_lowercase() {
            if lower.is_alphanumeric() || lower == '-' || lower == '_' {
                out.push(lower);
            } else if lower.is_whitespace() {
                out.push('-');
            }
        }
    }
    out
}

/// Normalizes a note/asset name or alias for case-insensitive, Turkish-aware lookup.
pub fn normalize_lookup_key(s: &str) -> String {
    let trimmed = s.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        match ch {
            'İ' | 'I' | 'ı' => out.push('i'),
            _ => {
                for lower in ch.to_lowercase() {
                    out.push(lower);
                }
            }
        }
    }
    out
}

/// Strips the first matching leading directory prefix (e.g. `content/`) from a relative path.
pub fn strip_prefix_path(rel_path: &str, prefixes: &[String]) -> String {
    for prefix in prefixes {
        let p = prefix.trim_matches('/');
        if p.is_empty() {
            continue;
        }
        if let Some(rest) = rel_path.strip_prefix(p) {
            if let Some(stripped) = rest.strip_prefix('/') {
                return stripped.to_string();
            }
        }
    }
    rel_path.to_string()
}

/// Percent-encodes each path segment (preserving `/`) for spaces and UTF-8 bytes in URLs.
pub fn url_encode_path(path: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(path.len() + 16);
    for (idx, seg) in path.split('/').enumerate() {
        if idx > 0 {
            out.push('/');
        }
        for &b in seg.as_bytes() {
            let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~');
            if unreserved {
                out.push(b as char);
            } else {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quartz_slugify_parity() {
        assert_eq!(slugify_file_path("Mantık Devreleri.md"), "Mantık-Devreleri");
        assert_eq!(slugify_file_path("Arkies & Okies.md"), "Arkies--and--Okies");
        assert_eq!(
            slugify_file_path("Math/Pumping Lemma/exercise.pdf"),
            "Math/Pumping-Lemma/exercise.pdf"
        );
        assert_eq!(
            slugify_file_path("Math/logic/çizelge.pdf"),
            "Math/logic/çizelge.pdf"
        );
    }

    #[test]
    fn test_strip_prefix_and_url_encode() {
        let prefixes = vec!["content".to_string()];
        assert_eq!(strip_prefix_path("content/index.md", &prefixes), "index.md");
        assert_eq!(
            strip_prefix_path("content/American_History_MOC.md", &prefixes),
            "American_History_MOC.md"
        );
        assert_eq!(
            strip_prefix_path("Math/induction_test.pdf", &prefixes),
            "Math/induction_test.pdf"
        );

        assert_eq!(
            url_encode_path("Math/Pumping Lemma/exercise.pdf"),
            "Math/Pumping%20Lemma/exercise.pdf"
        );
        assert_eq!(
            url_encode_path("Math/logic/çizelge.pdf"),
            "Math/logic/%C3%A7izelge.pdf"
        );
    }
}
