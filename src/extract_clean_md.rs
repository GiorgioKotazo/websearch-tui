//! HTML content extraction and Markdown conversion
//!
//! Uses Mozilla's Readability algorithm via readability-js for high-quality
//! content extraction, then converts to clean Markdown.
//!
//! Note: Readability uses QuickJS which is not Send/Sync, so we create
//! a new instance per extraction.

use anyhow::{Context, Result};
use html_to_markdown_rs::{
    convert, CodeBlockStyle, ConversionOptions, HeadingStyle, NewlineStyle, PreprocessingOptions,
    PreprocessingPreset,
};
use readability_js::Readability;

/// Extract clean content from HTML and convert to Markdown
pub fn extract_clean_markdown(html: &str, url: &str) -> Result<ExtractedContent> {
    // Create Readability instance (~30ms)
    let reader = Readability::new()
        .context("Failed to initialize Readability")?;
    
    // Extract main content (~10ms)
    let article = reader
        .parse_with_url(html, url)
        .context("Failed to parse HTML with Readability")?;

    // Configure html-to-markdown-rs options
    let options = create_markdown_options();

    // Convert cleaned HTML to Markdown
    let markdown = convert(&article.content, Some(options))
        .context("Failed to convert HTML to Markdown")?;

    Ok(ExtractedContent {
        title: article.title,
        byline: article.byline,
        excerpt: article.excerpt,
        site_name: article.site_name,
        markdown,
        url: url.to_string(),
    })
}

/// Create optimized Markdown conversion options
fn create_markdown_options() -> ConversionOptions {
    let mut options = ConversionOptions {
        heading_style: HeadingStyle::Atx,
        code_block_style: CodeBlockStyle::Backticks,
        newline_style: NewlineStyle::Backslash,
        list_indent_width: 2,
        bullets: "-".to_string(),
        strong_em_symbol: '*',
        escape_asterisks: false,
        escape_underscores: false,
        ..Default::default()
    };

    options.preprocessing = PreprocessingOptions {
        enabled: true,
        preset: PreprocessingPreset::Aggressive,
        remove_navigation: true,
        remove_forms: true,
        ..Default::default()
    };

    options.strip_tags = vec![
        "script".to_string(),
        "style".to_string(),
        "noscript".to_string(),
        "iframe".to_string(),
        "nav".to_string(),
        "aside".to_string(),
        "footer".to_string(),
        "header".to_string(),
    ];

    options
}

/// Structure containing extracted content
#[derive(Debug, Clone)]
pub struct ExtractedContent {
    pub title: String,
    pub byline: Option<String>,
    pub excerpt: Option<String>,
    pub site_name: Option<String>,
    pub markdown: String,
    pub url: String,
}

impl ExtractedContent {
    /// Format as Markdown with YAML frontmatter
    pub fn to_formatted_markdown(&self) -> String {
        let mut result = String::with_capacity(self.markdown.len() + 512);

        // YAML frontmatter for Neovim plugins
        result.push_str("---\n");
        result.push_str(&format!(
            "title: \"{}\"\n",
            self.title.replace('"', "\\\"")
        ));
        result.push_str(&format!("url: {}\n", self.url));
        if let Some(byline) = &self.byline {
            result.push_str(&format!("author: \"{}\"\n", byline.replace('"', "\\\"")));
        }
        if let Some(site_name) = &self.site_name {
            result.push_str(&format!("source: \"{}\"\n", site_name.replace('"', "\\\"")));
        }
        result.push_str("---\n\n");

        // Title
        result.push_str(&format!("# {}\n\n", self.title));

        // Metadata block
        if let Some(byline) = &self.byline {
            result.push_str(&format!("**Author**: {}\n", byline));
        }
        if let Some(site_name) = &self.site_name {
            result.push_str(&format!("**Source**: {}\n", site_name));
        }
        result.push_str(&format!("**URL**: <{}>\n\n", self.url));

        // Excerpt as blockquote
        if let Some(excerpt) = &self.excerpt {
            if !excerpt.is_empty() {
                result.push_str(&format!("> {}\n\n", excerpt));
            }
        }

        result.push_str("---\n\n");
        result.push_str(&self.markdown);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_article() {
        let html = r#"
            <html>
            <body>
                <article>
                    <h1>Test Title</h1>
                    <p>Test content with <strong>bold</strong> text.</p>
                </article>
            </body>
            </html>
        "#;

        let result = extract_clean_markdown(html, "https://test.com");
        assert!(result.is_ok());

        let content = result.unwrap();
        assert_eq!(content.title, "Test Title");
        assert!(content.markdown.contains("bold"));
    }
}
