/// Replicates Quartz 4.5.2's exact URL slug rules from `quartz/util/path.ts`
/// as well as `github-slugger` anchor slugging for `#heading` links.

/// Exact port of `sluggify(s: string)` in `quartz/util/path.ts`:
/// - replaces whitespace (`\s`) with `-`
/// - replaces `&` with `-and-`
/// - replaces `%` with `-percent`
/// - removes `?` and `#`
/// - preserves Unicode (Turkish characters like `ı`, `İ`, `ş`, `ğ`, `ü`, `ö`, `ç`) and case
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

/// Port of `slugifyFilePath(fp, excludeExt)` from `quartz/util/path.ts`.
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

/// Port of `github-slugger` used by Quartz for `#heading` anchors:
/// lowercases, removes punctuation except `-` and `_`, and turns spaces into `-`.
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

/// Normalizes a note name or alias for case-insensitive, Turkish-aware lookup.
/// Handles Turkish `İ`/`i` and `I`/`ı` as well as trimming leading/trailing whitespace.
pub fn normalize_lookup_key(s: &str) -> String {
    let trimmed = s.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        match ch {
            'İ' => out.push('i'),
            'I' => out.push('i'), // Allow ASCII 'I' and Turkish 'İ'/'ı' to fold gracefully in link targets
            'ı' => out.push('i'),
            _ => {
                for lower in ch.to_lowercase() {
                    out.push(lower);
                }
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
        assert_eq!(
            slugify_file_path("Mantık Devreleri.md"),
            "Mantık-Devreleri"
        );
        assert_eq!(
            slugify_file_path("Arkies & Okies.md"),
            "Arkies--and--Okies"
        );
        assert_eq!(
            slugify_file_path("1950's us Sylvia Plath.md"),
            "1950's-us-Sylvia-Plath"
        );
    }
}
