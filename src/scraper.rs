use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, ACCEPT_LANGUAGE};
use scraper::{Html, Selector};
use std::collections::{HashSet, VecDeque};
use std::time::Duration;
use log::{info, warn, error};
use url::Url;
use crate::extractor::Extractor;
use crate::delay_manager;

pub struct Scraper {
    client: Client,
    extractor: Extractor,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct Contact {
    pub name: Option<String>,
    pub title: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Default)]
pub struct ScrapingResult {
    pub emails: HashSet<String>,
    pub phones: HashSet<String>,
    pub contacts: Vec<Contact>, // Structured data
    pub status: ScrapeStatus,
    pub source_pages: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum ScrapeStatus {
    Success,
    NoData,
    Blocked,
    Error,
}

impl Default for ScrapeStatus {
    fn default() -> Self {
        ScrapeStatus::NoData
    }
}

impl Scraper {
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .cookie_store(true)
            .build()
            .expect("Failed to build HTTP client");

        Scraper {
            client,
            extractor: Extractor::new(),
        }
    }

    fn get_random_user_agent(&self) -> &str {
        let uas = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:121.0) Gecko/20100101 Firefox/121.0",
        ];
        use rand::Rng;
        let mut rng = rand::thread_rng();
        uas[rng.gen_range(0..uas.len())]
    }

    pub fn scrape_site(&self, start_url: &str) -> ScrapingResult {
        let mut result = ScrapingResult::default();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        
        // Normalize start URL
        let base_url = match Url::parse(start_url) {
            Ok(u) => u,
            Err(_) => {
                error!("Invalid URL: {}", start_url);
                result.status = ScrapeStatus::Error;
                return result;
            }
        };

        queue.push_back(start_url.to_string());
        let mut pages_visited = 0;
        let max_pages = 3;

        while let Some(url_str) = queue.pop_front() {
            if pages_visited >= max_pages {
                break;
            }
            if visited.contains(&url_str) {
                continue;
            }
            
            info!("Visiting: {}", url_str);
            
            // Random Delay before request (except maybe first? No, always be safe)
            if pages_visited > 0 {
                delay_manager::random_page_delay();
            }

            match self.visit_page(&url_str) {
                Ok((html_content, status_code)) => {
                    visited.insert(url_str.clone());
                    pages_visited += 1;

                    if status_code.as_u16() == 403 || status_code.as_u16() == 429 {
                        warn!("Blocked at {}: {}", url_str, status_code);
                        result.status = ScrapeStatus::Blocked;
                        return result; // Stop immediately if blocked
                    }

                    // --- NEW: Context-Aware Extraction ---
                    let document = Html::parse_document(&html_content);
                    // Select likely contact containers
                    let container_selector = Selector::parse("div, p, li, section, article, tr").unwrap();
                    
                    for container in document.select(&container_selector) {
                        // Split text by lines to keep context tight
                        let text_content = container.text().collect::<Vec<_>>().join("\n");
                        let lines: Vec<&str> = text_content.lines().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                        
                        // We iterate lines. If we find a phone, we look at the current line AND the previous line for Name/Title.
                        for (i, line) in lines.iter().enumerate() {
                             let phones = self.extractor.extract_phones(line);
                             if !phones.is_empty() {
                                 // Found phone in this line.
                                 // 1. Check THIS line for Name/Title
                                 let mut title = self.extractor.extract_job_title(line);
                                 let mut name = self.extractor.extract_name_candidate(line);
                                 let emails = self.extractor.extract_emails(line);

                                 // 2. If Name missing, check PREVIOUS line (common pattern: Name \n Phone)
                                 if name.is_none() && i > 0 {
                                     name = self.extractor.extract_name_candidate(lines[i-1]);
                                     // If we found name in prev line, maybe title is there too?
                                     if title.is_none() {
                                         title = self.extractor.extract_job_title(lines[i-1]);
                                     }
                                 }

                                 // 3. If Name still missing, check PREVIOUS-PREVIOUS line (Name \n Title \n Phone)
                                 if name.is_none() && i > 1 {
                                     name = self.extractor.extract_name_candidate(lines[i-2]);
                                 }
                                 
                                 if title.is_none() && i > 0 {
                                      // Sometimes title is on line above phone
                                      title = self.extractor.extract_job_title(lines[i-1]);
                                 }

                                 // Create contact if we have something useful beyond just a phone (or if phone is rare)
                                 // Actually, if we found a phone, we should record it. But "Contact" struct implies we know WHO it is.
                                 // If name is found, great. If title found, great.
                                 // If neither, maybe it's just a raw number, but we can assign title="Office" or something if generic?
                                 // User wants "Who is that".
                                 
                                 if name.is_some() || title.is_some() {
                                     let contact = Contact {
                                         name: name,
                                         title: title,
                                         phone: phones.iter().next().cloned(),
                                         email: emails.iter().next().cloned(),
                                     };
                                     
                                     let exists = result.contacts.iter().any(|c| 
                                         c.phone == contact.phone && c.name == contact.name
                                     );
                                     if !exists {
                                         result.contacts.push(contact);
                                     }
                                 }
                             }
                        }
                    }

                    // --- Global Fallback (Existing) ---
                    let emails = self.extractor.extract_emails(&html_content);
                    let phones = self.extractor.extract_phones(&html_content);
                    
                    if !emails.is_empty() || !phones.is_empty() {
                         result.source_pages.push(url_str.clone());
                    }

                    result.emails.extend(emails);
                    result.phones.extend(phones);

                    // Discover Links (only from homepage usually, or if queue is empty)
                    if pages_visited == 1 {
                        let discovered = self.discover_contact_links(&html_content, &base_url);
                        for link in discovered {
                            if !visited.contains(&link) {
                                queue.push_back(link);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch {}: {}", url_str, e);
                    // Don't error the whole site just for one page fail, unless it's the home page
                    if pages_visited == 0 {
                         result.status = ScrapeStatus::Error;
                         return result;
                    }
                }
            }
        }

        if !result.emails.is_empty() || !result.phones.is_empty() {
            result.status = ScrapeStatus::Success;
        } else if result.status != ScrapeStatus::Blocked && result.status != ScrapeStatus::Error {
            result.status = ScrapeStatus::NoData;
        }

        result
    }

    fn visit_page(&self, url: &str) -> Result<(String, reqwest::StatusCode), reqwest::Error> {
        let ua = self.get_random_user_agent();
        let resp = self.client.get(url)
            .header(USER_AGENT, ua)
            .send()?;
        
        let status = resp.status();
        let text = resp.text()?;
        Ok((text, status))
    }

    fn discover_contact_links(&self, html: &str, base_url: &Url) -> Vec<String> {
        let document = Html::parse_document(html);
        let selector = Selector::parse("a").unwrap();
        let mut links = Vec::new();

        for element in document.select(&selector) {
            if let Some(href) = element.value().attr("href") {
                let href_lower = href.to_lowercase();
                if href_lower.contains("contact") || href_lower.contains("about") {
                    if let Ok(joined_url) = base_url.join(href) {
                        // Ensure we stay on the same domain
                        if joined_url.domain().is_some() && joined_url.domain() == base_url.domain() {
                             links.push(joined_url.to_string());
                        }
                    }
                }
            }
        }
        // Deduplicate and limit
        links.sort();
        links.dedup();
        links.into_iter().take(2).collect() // limit to 2 contact pages found
    }
}
