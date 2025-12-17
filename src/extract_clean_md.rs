use readability_js::Readability;
use html_to_markdown_rs::{
    convert, 
    ConversionOptions, 
    HeadingStyle,
    PreprocessingOptions,
    PreprocessingPreset,
    CodeBlockStyle,
    NewlineStyle,
};
use anyhow::{Result, Context};

/// Extract clean content from HTML and convert to Markdown
pub fn extract_clean_markdown(html: &str, url: &str) -> Result<ExtractedContent> {
    // Step 1: Extract main content using readability-js
    let reader = Readability::new()
        .context("Failed to initialize Readability")?;
    
    let article = reader.parse_with_url(html, url)
        .context("Failed to parse HTML with Readability")?;
    
    // Step 2: Configure html-to-markdown-rs options with aggressive cleaning
    let mut options = ConversionOptions {
        heading_style: HeadingStyle::Atx,          // # style headings
        code_block_style: CodeBlockStyle::Backticks, // ``` code blocks
        newline_style: NewlineStyle::Backslash,     // \ for line breaks
        list_indent_width: 2,
        bullets: "-".to_string(),
        strong_em_symbol: '*',
        escape_asterisks: false,
        escape_underscores: false,
        ..Default::default()
    };
    
    // Step 3: Enable preprocessing for additional cleanup
    options.preprocessing = PreprocessingOptions {
        enabled: true,
        preset: PreprocessingPreset::Aggressive, // Aggressive noise removal
        remove_navigation: true,
        remove_forms: true,
        ..Default::default()
    };
    
    // Additional tags to strip
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
    
    // Step 4: Convert cleaned HTML to Markdown
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

/// Structure containing extracted content
#[derive(Debug, Clone)]
pub struct ExtractedContent {
    /// Article title
    pub title: String,
    /// Author (if found)
    pub byline: Option<String>,
    /// Brief description
    pub excerpt: Option<String>,
    /// Site name
    pub site_name: Option<String>,
    /// Clean Markdown content
    pub markdown: String,
    /// Source URL
    pub url: String,
}

impl ExtractedContent {
    /// Format result as nicely formatted Markdown with metadata
    pub fn to_formatted_markdown(&self) -> String {
        let mut result = String::new();
        
        // Title
        result.push_str(&format!("# {}\n\n", self.title));
        
        // Metadata
        if let Some(byline) = &self.byline {
            result.push_str(&format!("**Author**: {}\n", byline));
        }
        
        if let Some(site_name) = &self.site_name {
            result.push_str(&format!("**Source**: {}\n", site_name));
        }
        
        result.push_str(&format!("**URL**: {}\n\n", self.url));
        
        if let Some(excerpt) = &self.excerpt {
            result.push_str(&format!("> {}\n\n", excerpt));
        }
        
        result.push_str("---\n\n");
        
        // Main content
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
    
    #[test]
    fn test_removes_navigation() {
        let html = r#"
            <html>
            <body>
                <nav>Should be removed</nav>
                <article>
                    <h1>Title</h1>
                    <p>Content</p>
                </article>
            </body>
            </html>
        "#;
        
        let result = extract_clean_markdown(html, "https://test.com").unwrap();
        assert!(!result.markdown.contains("Should be removed"));
    }
}