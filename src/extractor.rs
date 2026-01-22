use regex::Regex;
use std::collections::HashSet;

pub struct Extractor {
    email_regex: Regex,
    phone_regex: Regex,
    indian_mobile_regex: Regex,
}

impl Extractor {
    pub fn new() -> Self {
        Extractor {
            // General email regex
            email_regex: Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap(),
            // General phone regex (International + India Landline/Mobile)
            phone_regex: Regex::new(r"(?:\+?\d{1,4}[-.\s]?)?(?:\(?\d{3}\)?[-.\s]?)?\d{3}[-.\s]?\d{4}").unwrap(),
            // Specific India Mobile regex for high confidence
            indian_mobile_regex: Regex::new(r"(?:\+91[\-\s]?)?[6-9]\d{9}").unwrap(),
        }
    }

    pub fn extract_emails(&self, text: &str) -> HashSet<String> {
        let mut emails = HashSet::new();
        for cap in self.email_regex.captures_iter(text) {
            if let Some(match_str) = cap.get(0) {
                let email = match_str.as_str().to_lowercase();
                if !email.ends_with(".png") && !email.ends_with(".jpg") && !email.ends_with(".jpeg") && !email.ends_with(".gif") && !email.ends_with(".webp") {
                     emails.insert(email);
                }
            }
        }
        emails
    }

    pub fn extract_phones(&self, text: &str) -> HashSet<String> {
        let mut phones = HashSet::new();
        
        // 1. Indian Mobiles (High Priority)
        for cap in self.indian_mobile_regex.captures_iter(text) {
             if let Some(match_str) = cap.get(0) {
                phones.insert(match_str.as_str().trim().to_string());
            }
        }

        // 2. Generic Phones
        for cap in self.phone_regex.captures_iter(text) {
            if let Some(match_str) = cap.get(0) {
                let p = match_str.as_str().trim().to_string();
                let digits: String = p.chars().filter(|c| c.is_digit(10)).collect();
                if digits.len() >= 10 && digits.len() <= 13 {
                     phones.insert(p);
                }
            }
        }
        phones
    }

    pub fn extract_job_title(&self, text: &str) -> Option<String> {
        let text_lower = text.to_lowercase();
        // Regex to capture the title phrase: (optional adjective) + title keyword + (optional suffix)
        // e.g. "Senior Sales Manager", "VP of Engineering"
        // This is complex to do purely with regex on unknown text, so stick to keyword matching but expand capture.
        
        let titles = [
            "ceo", "founder", "co-founder", "director", "manager", "president", 
            "vp", "vice president", "head of", "chief", "owner", "partner",
            "sales", "support", "representative", "consultant", "hr", "human resources",
            "executive", "officer", "admin", "administrator"
        ];

        for title in titles {
            if let Some(idx) = text_lower.find(title) {
                // Found a keyword. Try to grab surrounding context.
                // Grab up to 3 words before and 3 after?
                // Simpler: Grab the sentence or a chunk around it.
                
                // Let's try to grab the immediate phrase.
                // Find start of line or comma/period before.
                let start = text_lower[..idx].rfind(|c| ",.|:\n".contains(c)).map(|i| i+1).unwrap_or(0);
                let end = text_lower[idx..].find(|c| ",.|:\n".contains(c)).map(|i| idx+i).unwrap_or(text.len());
                
                let candidate = text[start..end].trim();
                // If candidate is too long (> 50 chars), it's probably a whole sentence, just return keyword.
                if candidate.len() < 50 && candidate.len() > title.len() {
                    return Some(candidate.to_string());
                }
                return Some(title.to_string()); // Fallback to keyword
            }
        }
        None
    }

    pub fn extract_name_candidate(&self, text: &str) -> Option<String> {
        // Banned generic names
        let banned = [
             "contact", "us", "touch", "support", "info", "customer", "service", "help", "desk", 
             "address", "phone", "email", "mobile", "office", "headquarters", "inquiry", "sales", 
             "admin", "webmaster", "career", "job", "opening", "team", "staff", "member", "department",
             "feedback", "question", "faq", "home", "about", "product", "privacy", "policy", "terms",
             "copyright", "rights", "reserved", "sitemap", "login", "register", "sign", "up"
        ];

        let words: Vec<&str> = text.split_whitespace().collect();
        // Name usually 2-3 words.
        if words.len() < 2 || words.len() > 3 { return None; }

        let is_capitalized = words.iter().all(|w| {
            let first = w.chars().next().unwrap_or('a');
            first.is_uppercase() && w.chars().any(|c| c.is_lowercase()) 
        });

        if !is_capitalized { return None; }

        let candidate_lower = text.to_lowercase();
        for ban in banned {
            if candidate_lower.contains(ban) {
                return None; // Contains a banned word
            }
        }

        Some(text.to_string())
    }
}
