use anyhow::{Context, Result};
use dom_smoothie::{
    CandidateSelectMode, Config, ParsePolicy, Readability, TextMode,
};

/// Структура с извлечённым контентом
#[derive(Debug, Clone)]
pub struct ExtractedContent {
    pub title: String,
    pub byline: Option<String>,
    pub excerpt: Option<String>,
    pub site_name: Option<String>,
    pub markdown: String,
    pub url: String,
}

/// Извлечь чистый контент из HTML и сконвертировать в Markdown
pub fn extract_clean_markdown(html: &str, url: &str) -> Result<ExtractedContent> {
    // Правильная настройка конфигурации
    let config = Config {
        text_mode: TextMode::Markdown,
        candidate_select_mode: CandidateSelectMode::Readability,
        ..Default::default()
    };

    // Readability::new возвращает Result<Readability, ReadabilityError>
    let mut readability = Readability::new(html, Some(url), Some(config))
        .context("Failed to create Readability instance")?;

    // Правильный метод: parse_with_policy (или просто parse() в новых версиях)
    let article = readability
        .parse_with_policy(ParsePolicy::Strict)
        .context("Failed to parse HTML with dom_smoothie")?;

    // Возвращаем структуру с извлечённым контентом
    Ok(ExtractedContent {
        title: article.title,
        byline: article.byline,
        excerpt: article.excerpt,
        site_name: article.site_name,
        markdown: article.text_content.to_string(), // Уже в Markdown формате
        url: url.to_string(),
    })
}

impl ExtractedContent {
    /// Отформатировать в Markdown с YAML frontmatter
    pub fn to_formatted_markdown(&self) -> String {
        let mut result = String::new();

        // YAML frontmatter
        result.push_str("---\n");
        result.push_str(&format!("title: \"{}\"\n", self.title.replace('"', "\\\"")));
        result.push_str(&format!("url: {}\n", self.url));

        if let Some(ref byline) = self.byline {
            result.push_str(&format!("author: \"{}\"\n", byline.replace('"', "\\\"")));
        }

        if let Some(ref site_name) = self.site_name {
            result.push_str(&format!("source: \"{}\"\n", site_name.replace('"', "\\\"")));
        }
        result.push_str("---\n\n");

        // Заголовок
        result.push_str(&format!("# {}\n\n", self.title));

        // Метаданные
        if let Some(ref byline) = self.byline {
            result.push_str(&format!("**Author**: {}\n", byline));
        }
        if let Some(ref site_name) = self.site_name {
            result.push_str(&format!("**Source**: {}\n", site_name));
        }
        result.push_str(&format!("**URL**: [{}]({})\n\n", self.title, self.url));

        // Excerpt
        if let Some(ref excerpt) = self.excerpt {
            if !excerpt.is_empty() {
                result.push_str(&format!("> {}\n\n", excerpt.trim()));
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
            <head><title>Test Title</title></head>
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
        assert!(!content.markdown.is_empty());
    }
}
