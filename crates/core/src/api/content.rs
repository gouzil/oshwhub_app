use super::response::web_url;
use crate::{
    error::{AppError, Result},
    model::ContentBlock,
};
use scraper::ElementRef;

pub(super) fn content_blocks(html: &str) -> Result<Vec<ContentBlock>> {
    if html.len() > 2 * 1024 * 1024 {
        return Err(AppError::new("CONTENT_LIMIT", "工程正文超过安全大小上限"));
    }
    let fragment = scraper::Html::parse_fragment(html);
    let mut blocks = Vec::new();
    let mut text = String::new();
    walk_html(fragment.root_element(), 0, &mut blocks, &mut text)?;
    flush_text(&mut blocks, &mut text)?;
    Ok(blocks)
}

fn excluded(tag: &str) -> bool {
    matches!(
        tag,
        "script"
            | "style"
            | "iframe"
            | "object"
            | "embed"
            | "noscript"
            | "template"
            | "svg"
            | "canvas"
    )
}

fn inline_text(element: ElementRef<'_>, depth: usize) -> Result<String> {
    check_depth(depth)?;
    let mut text = String::new();
    for node in element.children() {
        if let Some(value) = node.value().as_text() {
            text.push_str(value);
        } else if let Some(child) = ElementRef::wrap(node)
            && !excluded(child.value().name())
        {
            if child.value().name() == "br" {
                text.push('\n');
            } else {
                text.push_str(&inline_text(child, depth + 1)?);
            }
        }
    }
    Ok(text)
}

fn check_depth(depth: usize) -> Result<()> {
    if depth > 128 {
        return Err(AppError::new("CONTENT_LIMIT", "工程正文嵌套过深"));
    }
    Ok(())
}

fn push_block(
    blocks: &mut Vec<ContentBlock>,
    kind: &str,
    text: String,
    url: Option<String>,
) -> Result<()> {
    let text = if kind == "code" {
        text.trim_matches('\n').to_owned()
    } else {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    if text.is_empty() && kind != "image" {
        return Ok(());
    }
    if blocks.len() >= 2048 {
        return Err(AppError::new("CONTENT_LIMIT", "工程正文内容块过多"));
    }
    blocks.push(ContentBlock {
        kind: kind.to_owned(),
        text,
        url,
    });
    Ok(())
}

fn flush_text(blocks: &mut Vec<ContentBlock>, text: &mut String) -> Result<()> {
    push_block(blocks, "paragraph", std::mem::take(text), None)
}

fn walk_html(
    element: ElementRef<'_>,
    depth: usize,
    blocks: &mut Vec<ContentBlock>,
    text: &mut String,
) -> Result<()> {
    check_depth(depth)?;
    for node in element.children() {
        if let Some(value) = node.value().as_text() {
            text.push_str(value);
            continue;
        }
        let Some(child) = ElementRef::wrap(node) else {
            continue;
        };
        let tag = child.value().name();
        if excluded(tag) {
            continue;
        }
        match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "pre" => {
                flush_text(blocks, text)?;
                let kind = if tag == "pre" { "code" } else { "heading" };
                push_block(blocks, kind, inline_text(child, depth + 1)?, None)?;
            }
            "img" => {
                flush_text(blocks, text)?;
                if let Some(url) = child
                    .value()
                    .attr("src")
                    .and_then(|s| web_url(s, "https://oshwhub.com"))
                {
                    push_block(
                        blocks,
                        "image",
                        child.value().attr("alt").unwrap_or_default().to_owned(),
                        Some(url),
                    )?;
                }
            }
            "a" => {
                if child.descendants().any(|node| {
                    node.value()
                        .as_element()
                        .is_some_and(|element| element.name() == "img")
                }) {
                    flush_text(blocks, text)?;
                    walk_html(child, depth + 1, blocks, text)?;
                    continue;
                }
                let label = inline_text(child, depth + 1)?;
                if let Some(url) = child
                    .value()
                    .attr("href")
                    .and_then(|s| web_url(s, "https://oshwhub.com"))
                {
                    flush_text(blocks, text)?;
                    let label = if label.trim().is_empty() {
                        url.clone()
                    } else {
                        label
                    };
                    push_block(blocks, "link", label, Some(url))?;
                } else {
                    text.push_str(&label);
                }
            }
            "br" => text.push('\n'),
            "div" | "section" | "article" | "p" | "li" | "blockquote" | "ul" | "ol" | "table"
            | "thead" | "tbody" | "tr" | "td" | "th" | "figure" | "figcaption" | "header"
            | "footer" | "main" | "hr" => {
                flush_text(blocks, text)?;
                let mut child_text = String::new();
                walk_html(child, depth + 1, blocks, &mut child_text)?;
                flush_text(blocks, &mut child_text)?;
            }
            _ => walk_html(child, depth + 1, blocks, text)?,
        }
    }
    Ok(())
}
