use anyhow::{anyhow, Context, Result};
use roxmltree::Document;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use zip::ZipArchive;

/// Canonical chunk size in UTF-8 bytes per synthetic page (matching Adobe Digital Editions / Calibre convention)
pub const CANONICAL_PAGE_BYTES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpubBlock {
    Heading { level: u8, text: String, id: Option<String> },
    Paragraph { text: String, id: Option<String> },
    Blockquote { text: String, id: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpineChapter {
    pub spine_index: usize,
    pub id: String,
    pub href: String,
    pub title: String,
    pub blocks: Vec<EpubBlock>,
    pub plain_text: String,
    pub start_page: usize,
    pub end_page: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalPage {
    pub page_number: usize,
    pub spine_index: usize,
    pub chapter_title: String,
    pub char_offset_in_chapter: usize,
    pub preview_snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubBook {
    pub file_name: String,
    pub title: String,
    pub authors: Vec<String>,
    pub language: Option<String>,
    pub identifier: Option<String>,
    pub blake3_hash: String,
    pub chapters: Vec<SpineChapter>,
    pub pages: Vec<CanonicalPage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub file_name: String,
    pub title: String,
    pub authors: Vec<String>,
    pub total_pages: usize,
    pub blake3_hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookCatalog {
    pub version: u32,
    pub books: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubBacklink {
    pub note_path: String,
    pub line: usize,
    pub book_file: String,
    pub page: Option<usize>,
    pub chapter: Option<String>,
    pub cfi: Option<String>,
    pub display_alias: Option<String>,
}

impl EpubBook {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "book.epub".to_string());
        let bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read EPUB file at {}", path.display()))?;
        let hash = blake3::hash(&bytes).to_hex().to_string();
        let file = File::open(path)?;
        Self::from_reader(file, file_name, hash)
    }

    pub fn from_reader<R: Read + Seek>(
        reader: R,
        file_name: String,
        blake3_hash: String,
    ) -> Result<Self> {
        let mut archive = ZipArchive::new(reader).context("Invalid EPUB ZIP archive")?;

        // 1. Parse META-INF/container.xml
        let container_xml = read_zip_string(&mut archive, "META-INF/container.xml")?;
        let opf_path = parse_container_rootfile(&container_xml)?;
        let opf_dir = match opf_path.rfind('/') {
            Some(idx) => &opf_path[..=idx],
            None => "",
        };

        // 2. Parse OPF package document
        let opf_xml = read_zip_string(&mut archive, &opf_path)?;
        let opf_info = parse_opf_document(&opf_xml)?;

        // 3. Parse each spine XHTML item and build deterministic canonical pages
        let mut chapters = Vec::new();
        let mut pages = Vec::new();
        let mut current_page_num = 1usize;

        for (spine_index, itemref_id) in opf_info.spine_ids.iter().enumerate() {
            let Some(rel_href) = opf_info.manifest.get(itemref_id) else {
                continue;
            };
            let full_href = format!("{}{}", opf_dir, rel_href);
            let Ok(xhtml) = read_zip_string(&mut archive, &full_href) else {
                continue;
            };
            let (chapter_title, blocks, plain_text) =
                extract_xhtml_blocks(&xhtml, spine_index + 1);

            let start_page = current_page_num;
            let chapter_pages = paginate_text_deterministically(
                &plain_text,
                spine_index,
                &chapter_title,
                &mut current_page_num,
            );
            let end_page = (current_page_num - 1).max(start_page);
            pages.extend(chapter_pages);

            chapters.push(SpineChapter {
                spine_index,
                id: itemref_id.clone(),
                href: full_href,
                title: chapter_title,
                blocks,
                plain_text,
                start_page,
                end_page,
            });
        }

        Ok(Self {
            file_name,
            title: opf_info.title.unwrap_or_else(|| "Untitled Book".to_string()),
            authors: opf_info.authors,
            language: opf_info.language,
            identifier: opf_info.identifier,
            blake3_hash,
            chapters,
            pages,
        })
    }

    pub fn total_pages(&self) -> usize {
        self.pages.len()
    }

    pub fn resolve_page(&self, page_number: usize) -> Option<&CanonicalPage> {
        if page_number == 0 || page_number > self.pages.len() {
            None
        } else {
            self.pages.get(page_number - 1)
        }
    }

    /// Formats a selected quote from a given page/CFI into a Markdown blockquote + wikilink
    /// suitable for inserting into the active note in split view.
    pub fn format_quote_citation(
        &self,
        quote_text: &str,
        page_number: usize,
        cfi: Option<&str>,
    ) -> String {
        let author_prefix = if self.authors.is_empty() {
            String::new()
        } else {
            format!("{}, ", self.authors.join(" & "))
        };
        let target_frag = match cfi {
            Some(c) => format!("#p={}&cfi={}", page_number, c),
            None => format!("#p={}", page_number),
        };
        format!(
            "> \"{}\"\n> — [[{}{} | {}{}, s. {}]]",
            quote_text.trim(),
            self.file_name,
            target_frag,
            author_prefix,
            self.title,
            page_number
        )
    }

    pub fn to_catalog_entry(&self) -> CatalogEntry {
        CatalogEntry {
            file_name: self.file_name.clone(),
            title: self.title.clone(),
            authors: self.authors.clone(),
            total_pages: self.total_pages(),
            blake3_hash: self.blake3_hash.clone(),
        }
    }
}

fn read_zip_string<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str) -> Result<String> {
    let mut file = archive
        .by_name(name)
        .with_context(|| format!("Missing entry '{}' in EPUB archive", name))?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn parse_container_rootfile(xml: &str) -> Result<String> {
    let doc = Document::parse(xml).context("Failed to parse META-INF/container.xml")?;
    for node in doc.descendants() {
        if node.has_tag_name("rootfile") {
            if let Some(full_path) = node.attribute("full-path") {
                return Ok(full_path.to_string());
            }
        }
    }
    Err(anyhow!("No <rootfile full-path=...> found in container.xml"))
}

struct OpfData {
    title: Option<String>,
    authors: Vec<String>,
    language: Option<String>,
    identifier: Option<String>,
    manifest: BTreeMap<String, String>,
    spine_ids: Vec<String>,
}

fn parse_opf_document(xml: &str) -> Result<OpfData> {
    let doc = Document::parse(xml).context("Failed to parse OPF XML")?;
    let mut title = None;
    let mut authors = Vec::new();
    let mut language = None;
    let mut identifier = None;
    let mut manifest = BTreeMap::new();
    let mut spine_ids = Vec::new();

    for node in doc.descendants() {
        let tag = node.tag_name().name();
        match tag {
            "title" if title.is_none() => {
                if let Some(t) = node.text() {
                    if !t.trim().is_empty() {
                        title = Some(t.trim().to_string());
                    }
                }
            }
            "creator" => {
                if let Some(a) = node.text() {
                    if !a.trim().is_empty() {
                        authors.push(a.trim().to_string());
                    }
                }
            }
            "language" if language.is_none() => {
                if let Some(l) = node.text() {
                    language = Some(l.trim().to_string());
                }
            }
            "identifier" if identifier.is_none() => {
                if let Some(id) = node.text() {
                    identifier = Some(id.trim().to_string());
                }
            }
            "item" => {
                if let (Some(id), Some(href)) = (node.attribute("id"), node.attribute("href")) {
                    manifest.insert(id.to_string(), href.to_string());
                }
            }
            "itemref" => {
                if let Some(idref) = node.attribute("idref") {
                    spine_ids.push(idref.to_string());
                }
            }
            _ => {}
        }
    }

    Ok(OpfData {
        title,
        authors,
        language,
        identifier,
        manifest,
        spine_ids,
    })
}

fn extract_xhtml_blocks(xhtml: &str, default_chapter_num: usize) -> (String, Vec<EpubBlock>, String) {
    let mut blocks = Vec::new();
    let mut chapter_title = None;
    let mut full_text = String::new();

    if let Ok(doc) = Document::parse(xhtml) {
        for node in doc.descendants() {
            let tag = node.tag_name().name();
            let id = node.attribute("id").map(|s| s.to_string());
            match tag {
                "h1" | "h2" | "h3" | "h4" => {
                    let level = tag[1..].parse::<u8>().unwrap_or(2);
                    let text = collect_node_text(node);
                    if !text.is_empty() {
                        if chapter_title.is_none() {
                            chapter_title = Some(text.clone());
                        }
                        if !full_text.is_empty() {
                            full_text.push_str("\n\n");
                        }
                        full_text.push_str(&text);
                        blocks.push(EpubBlock::Heading { level, text, id });
                    }
                }
                "p" => {
                    let text = collect_node_text(node);
                    if !text.is_empty() {
                        if !full_text.is_empty() {
                            full_text.push_str("\n\n");
                        }
                        full_text.push_str(&text);
                        blocks.push(EpubBlock::Paragraph { text, id });
                    }
                }
                "blockquote" => {
                    let text = collect_node_text(node);
                    if !text.is_empty() {
                        if !full_text.is_empty() {
                            full_text.push_str("\n\n");
                        }
                        full_text.push_str(&text);
                        blocks.push(EpubBlock::Blockquote { text, id });
                    }
                }
                _ => {}
            }
        }
    }

    let title = chapter_title.unwrap_or_else(|| format!("Chapter {}", default_chapter_num));
    (title, blocks, full_text)
}

fn collect_node_text(node: roxmltree::Node) -> String {
    let mut buf = String::new();
    for desc in node.descendants() {
        if desc.is_text() {
            if let Some(t) = desc.text() {
                buf.push_str(t);
            }
        }
    }
    buf.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn paginate_text_deterministically(
    plain_text: &str,
    spine_index: usize,
    chapter_title: &str,
    next_page_num: &mut usize,
) -> Vec<CanonicalPage> {
    if plain_text.is_empty() {
        let p = CanonicalPage {
            page_number: *next_page_num,
            spine_index,
            chapter_title: chapter_title.to_string(),
            char_offset_in_chapter: 0,
            preview_snippet: String::new(),
        };
        *next_page_num += 1;
        return vec![p];
    }

    let mut pages = Vec::new();
    let mut byte_cursor = 0usize;
    let mut char_cursor = 0usize;
    let total_bytes = plain_text.len();

    while byte_cursor < total_bytes {
        let mut chunk_end = (byte_cursor + CANONICAL_PAGE_BYTES).min(total_bytes);
        while chunk_end < total_bytes && !plain_text.is_char_boundary(chunk_end) {
            chunk_end += 1;
        }
        let slice = &plain_text[byte_cursor..chunk_end];
        let preview: String = slice.chars().take(80).collect();
        pages.push(CanonicalPage {
            page_number: *next_page_num,
            spine_index,
            chapter_title: chapter_title.to_string(),
            char_offset_in_chapter: char_cursor,
            preview_snippet: preview.trim().to_string(),
        });
        *next_page_num += 1;
        char_cursor += slice.chars().count();
        byte_cursor = chunk_end;
    }

    pages
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn build_sample_epub_bytes() -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default();

            zip.start_file("META-INF/container.xml", opts).unwrap();
            std::io::Write::write_all(
                &mut zip,
                br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
            )
            .unwrap();

            zip.start_file("OEBPS/content.opf", opts).unwrap();
            std::io::Write::write_all(
                &mut zip,
                r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Mantık</dc:title>
    <dc:creator>Graham Priest</dc:creator>
    <dc:language>tr</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#
                    .as_bytes(),
            )
            .unwrap();

            let long_para = "Çözümleyici çizelgeler (truth trees) geçerliliği test etmek için kullanılır. ".repeat(30);
            let ch1_xhtml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<body>
  <h1 id="sec1">Bölüm 1: Geçerlilik ve Çözümleyici Çizelge</h1>
  <p id="p1">{}</p>
</body>
</html>"#,
                long_para
            );
            zip.start_file("OEBPS/ch1.xhtml", opts).unwrap();
            std::io::Write::write_all(&mut zip, ch1_xhtml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn test_epub_deterministic_pagination_and_citation() {
        let epub_bytes = build_sample_epub_bytes();
        let hash = blake3::hash(&epub_bytes).to_hex().to_string();
        let book = EpubBook::from_reader(
            Cursor::new(epub_bytes),
            "Priest - Mantık.epub".to_string(),
            hash,
        )
        .unwrap();

        assert_eq!(book.title, "Mantık");
        assert_eq!(book.authors, vec!["Graham Priest".to_string()]);
        assert!(book.total_pages() >= 2);

        let page_2 = book.resolve_page(2).unwrap();
        assert_eq!(page_2.page_number, 2);
        assert_eq!(
            page_2.chapter_title,
            "Bölüm 1: Geçerlilik ve Çözümleyici Çizelge"
        );

        let cite = book.format_quote_citation(
            "Çözümleyici çizelgeler geçerliliği test etmek için kullanılır.",
            2,
            Some("/6/2!/4/4"),
        );
        assert!(cite.contains("[[Priest - Mantık.epub#p=2&cfi=/6/2!/4/4 | Graham Priest, Mantık, s. 2]]"));
    }
}
