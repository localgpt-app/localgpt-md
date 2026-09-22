//! Markdown → [`Doc`]: a title, front matter, and one [`Section`] per `##`
//! heading — the unit that becomes a place in the world.
//!
//! Pure (no Bevy), like [`crate::draft`], so both can move into a shared
//! crate later (PLAN.md M5).

use std::collections::BTreeMap;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// A parsed Markdown document.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    /// The first `#` heading, or the caller's fallback (the file stem).
    pub title: String,
    /// `key: value` lines from a leading `---` block, e.g. `genre: deck`.
    pub front_matter: BTreeMap<String, String>,
    /// Prose before the first section.
    pub intro: String,
    /// One per `##` heading (and any `#` after the title), in document order.
    pub sections: Vec<Section>,
}

/// One section of prose, keyed by a content hash.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub heading: String,
    /// Paragraphs, list items, and `###` sub-headings as plain text, one per
    /// line. Code blocks are left out — they aren't scenery.
    pub body: String,
    /// BLAKE3 of heading + body: seeds the draft today and keys the LLM cache
    /// later, so an unchanged section never regenerates.
    pub hash: blake3::Hash,
}

impl Section {
    fn new(heading: String, body: &str) -> Self {
        let body = body.trim().to_string();
        let hash = blake3::hash(format!("{heading}\n{body}").as_bytes());
        Self {
            heading,
            body,
            hash,
        }
    }

    /// A stable 64-bit seed taken from [`Self::hash`].
    pub fn seed(&self) -> u64 {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&self.hash.as_bytes()[..8]);
        u64::from_le_bytes(bytes)
    }
}

impl Doc {
    /// Parse Markdown. A document without `##` headings becomes one section.
    pub fn parse(src: &str, fallback_title: &str) -> Self {
        let options = Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
            | Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS;

        let mut title = None;
        let mut front_matter = BTreeMap::new();
        let mut intro = String::new();
        let mut sections: Vec<(String, String)> = Vec::new();
        // The heading being read, if inside one.
        let mut heading: Option<(HeadingLevel, String)> = None;
        let mut in_metadata = false;
        let mut in_code = false;

        for event in Parser::new_ext(src, options) {
            match event {
                Event::Start(Tag::MetadataBlock(_)) => in_metadata = true,
                Event::End(TagEnd::MetadataBlock(_)) => in_metadata = false,
                Event::Start(Tag::CodeBlock(_)) => in_code = true,
                Event::End(TagEnd::CodeBlock) => in_code = false,
                Event::Text(text) if in_metadata => parse_front_matter(&text, &mut front_matter),
                Event::Start(Tag::Heading { level, .. }) => heading = Some((level, String::new())),
                Event::End(TagEnd::Heading(_)) => {
                    let Some((level, text)) = heading.take() else {
                        continue;
                    };
                    let text = text.trim().to_string();
                    match level {
                        HeadingLevel::H1 if title.is_none() && sections.is_empty() => {
                            title = Some(text);
                        }
                        HeadingLevel::H1 | HeadingLevel::H2 => sections.push((text, String::new())),
                        _ => {
                            let body = current_body(&mut intro, &mut sections);
                            body.push_str(&text);
                            body.push('\n');
                        }
                    }
                }
                Event::Text(text) | Event::Code(text) if !in_code => match heading.as_mut() {
                    Some((_, h)) => h.push_str(&text),
                    None => current_body(&mut intro, &mut sections).push_str(&text),
                },
                Event::SoftBreak | Event::End(TagEnd::TableCell) => match heading.as_mut() {
                    Some((_, h)) => h.push(' '),
                    None => current_body(&mut intro, &mut sections).push(' '),
                },
                Event::HardBreak
                | Event::End(
                    TagEnd::Paragraph | TagEnd::Item | TagEnd::TableRow | TagEnd::TableHead,
                ) if heading.is_none() => end_line(current_body(&mut intro, &mut sections)),
                _ => {}
            }
        }

        let title = title.unwrap_or_else(|| fallback_title.to_string());
        let intro = intro.trim().to_string();
        let mut sections: Vec<Section> = sections
            .into_iter()
            .map(|(heading, body)| Section::new(heading, &body))
            .collect();
        if sections.is_empty() && !intro.is_empty() {
            sections.push(Section::new(title.clone(), &intro));
        }
        Self {
            title,
            front_matter,
            intro,
            sections,
        }
    }

    /// The `genre` front-matter key, `world` when absent. Only `world` is
    /// implemented so far; `deck` is next (PLAN.md M4).
    pub fn genre(&self) -> &str {
        self.front_matter
            .get("genre")
            .map_or("world", String::as_str)
    }
}

/// The text currently being appended to: the last section, or the intro.
fn current_body<'a>(intro: &'a mut String, sections: &'a mut [(String, String)]) -> &'a mut String {
    match sections.last_mut() {
        Some((_, body)) => body,
        None => intro,
    }
}

fn end_line(body: &mut String) {
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
}

/// `key: value` lines; surrounding quotes are dropped and `#` lines skipped.
fn parse_front_matter(text: &str, out: &mut BTreeMap<String, String>) {
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            out.insert(key.trim().to_string(), value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---
genre: deck
note: \"quoted\"
---

# The Title

Intro line.

## First

A paragraph
that wraps.

- one
- two

### Detail

More.

```rust
let code = 1;
```

## Second

Text with `inline` code.
";

    #[test]
    fn title_front_matter_and_sections() {
        let doc = Doc::parse(SAMPLE, "fallback");
        assert_eq!(doc.title, "The Title");
        assert_eq!(doc.genre(), "deck");
        assert_eq!(doc.front_matter["note"], "quoted");
        assert_eq!(doc.intro, "Intro line.");
        let headings: Vec<_> = doc.sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(headings, ["First", "Second"]);
        assert_eq!(
            doc.sections[0].body,
            "A paragraph that wraps.\none\ntwo\nDetail\nMore."
        );
        assert_eq!(doc.sections[1].body, "Text with inline code.");
    }

    #[test]
    fn no_headings_is_one_section_named_by_fallback() {
        let doc = Doc::parse("Just a note.\n\nTwo paragraphs.", "note");
        assert_eq!(doc.title, "note");
        assert_eq!(doc.genre(), "world");
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].heading, "note");
        assert_eq!(doc.sections[0].body, "Just a note.\nTwo paragraphs.");
    }

    #[test]
    fn hash_tracks_each_section() {
        let before = Doc::parse(SAMPLE, "t");
        let after = Doc::parse(&SAMPLE.replace("More.", "Much more."), "t");
        assert_ne!(before.sections[0].hash, after.sections[0].hash);
        assert_eq!(before.sections[1].hash, after.sections[1].hash);
        assert_eq!(before.sections[1].seed(), after.sections[1].seed());
    }

    #[test]
    fn cjk_text_survives() {
        let doc = Doc::parse("# 花园\n\n## 第一章\n\n雾。\n", "t");
        assert_eq!(doc.title, "花园");
        assert_eq!(doc.sections[0].heading, "第一章");
        assert_eq!(doc.sections[0].body, "雾。");
    }
}
