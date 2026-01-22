use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};
use std::time::Duration;
use log::{info, warn, error};
use crate::delay_manager;

pub struct SearchEngine {
    client: Client,
}

impl SearchEngine {
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"));

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .cookie_store(true)
            .build()
            .expect("Failed to build Search Client");

        SearchEngine { client }
    }

    pub fn search_company(&self, company: &str, country: &str) -> Option<String> {
        // Construct query: "Company Country official website"
        let query = format!("{} {} official website", company, country);
        let encoded_query = urlencoding::encode(&query);
        let search_url = format!("https://html.duckduckgo.com/html/?q={}", encoded_query);

        info!("Searching for: '{}'", query);
        
        // Random delay to respect search engine
        delay_manager::random_page_delay();

        match self.client.get(&search_url).send() {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!("Search failed with status: {}", resp.status());
                    return None;
                }
                
                let text = match resp.text() {
                    Ok(t) => t,
                    Err(e) => {
                        error!("Failed to read search response: {}", e);
                        return None;
                    }
                };

                self.parse_duckduckgo_results(&text)
            }
            Err(e) => {
                error!("Search request failed: {}", e);
                None
            }
        }
    }

    fn parse_duckduckgo_results(&self, html: &str) -> Option<String> {
        let document = Html::parse_document(html);
        
        let forbidden_domains = [
            "facebook.com", "instagram.com", "linkedin.com", "twitter.com", "x.com", 
            "youtube.com", "pinterest.com", "glassdoor.com", "indeed.com",
            "justdial.com", "indiamart.com", "yellowpages.com"
        ];

        // DDG HTML uses specific classes. .result__a is the link title.
        // Try primary selector
        let selectors = [".result__a", ".result__snippet", ".result__url"];
        
        for sel_str in selectors {
            let selector = Selector::parse(sel_str).unwrap();
            for element in document.select(&selector) {
                if let Some(href) = element.value().attr("href") {
                    // Determine if this is a good URL
                    let skip = forbidden_domains.iter().any(|&d| href.contains(d));
                    
                    if !skip && href.starts_with("http") && !href.contains("duckduckgo.com") {
                        info!("Found likely Website using selector '{}': {}", sel_str, href);
                        return Some(href.to_string());
                    }
                }
            }
        }
        
        warn!("No suitable website found in top results.");
        None

    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_parsing() {
        // Live test against DuckDuckGo
        let engine = SearchEngine::new();
        let result = engine.search_company("Rust Foundation", "USA");
        assert!(result.is_some());
        let url = result.unwrap();
        // DuckDuckGo might redirect or give main page. 
        // foundation.rust-lang.org or rust-lang.org are both valid success indicators.
        assert!(url.contains("rust-lang")); 
    }
}
