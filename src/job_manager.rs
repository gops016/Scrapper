use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;
use business_scraper_lib::{Scraper, SearchEngine, InputRecord, ScrapeStatus, input_loader};
use std::path::PathBuf;
use std::fs::OpenOptions;
use csv::Writer;
use chrono::Local;

#[derive(Clone, serde::Serialize)]
pub struct ExtractedData {
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    pub contacts: Vec<business_scraper_lib::scraper::Contact>,
}

#[derive(Clone, serde::Serialize)]
pub struct JobStatus {
    pub id: String,
    pub status: String, // "queued", "processing", "paused", "stopped", "completed", "failed"
    pub total_records: usize,
    pub processed_count: usize,
    pub current_company: String,
    pub logs: Vec<String>,
    pub last_extracted: Option<ExtractedData>,
    #[serde(skip)]
    pub control_req: String, // "none", "pause", "time_to_stop"
}

pub struct JobManager {
    pub jobs: Arc<Mutex<HashMap<String, JobStatus>>>,
}

impl JobManager {
    pub fn new() -> Self {
        JobManager {
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_job(&self, job_id: String, input_path: PathBuf, output_path: PathBuf) -> String {
        let initial_status = JobStatus {
            id: job_id.clone(),
            status: "queued".to_string(),
            total_records: 0,
            processed_count: 0,
            current_company: "Initializing...".to_string(),
            logs: vec!["Job started.".to_string()],
            last_extracted: None,
            control_req: "none".to_string(),
        };

        self.jobs.lock().unwrap().insert(job_id.clone(), initial_status);

        let jobs_arc = self.jobs.clone();
        let id_clone = job_id.clone();

        thread::spawn(move || {
            Self::run_scraper(id_clone, jobs_arc, input_path, output_path);
        });

        job_id
    }

    pub fn send_control(&self, job_id: &str, signal: &str) -> bool {
        let mut guard = self.jobs.lock().unwrap();
        if let Some(job) = guard.get_mut(job_id) {
            match signal {
                "pause" => job.control_req = "pause".to_string(),
                "resume" => {
                    job.control_req = "none".to_string();
                    if job.status == "paused" {
                        job.status = "processing".to_string(); // Immediate feedback
                    }
                },
                "stop" => job.control_req = "stop".to_string(),
                _ => return false,
            }
            return true;
        }
        false
    }

    fn run_scraper(job_id: String, jobs: Arc<Mutex<HashMap<String, JobStatus>>>, input_path: PathBuf, output_path: PathBuf) {
        let update_status = |status: &str, company: &str, log: Option<String>, data: Option<ExtractedData>| {
            let mut guard = jobs.lock().unwrap();
            if let Some(job) = guard.get_mut(&job_id) {
                if !status.is_empty() { job.status = status.to_string(); }
                if !company.is_empty() { job.current_company = company.to_string(); }
                if let Some(msg) = log {
                    job.logs.push(msg);
                    if job.logs.len() > 50 { job.logs.remove(0); }
                }
                if let Some(d) = data {
                    job.last_extracted = Some(d);
                }
            }
        };

        // Load Records
        let input_str = input_path.to_str().unwrap_or("input.csv");
        let records = input_loader::load_records(input_str);
        
        {
            let mut guard = jobs.lock().unwrap();
            if let Some(job) = guard.get_mut(&job_id) {
                job.total_records = records.len();
                job.status = "processing".to_string();
            }
        }

        let scraper_instance = Scraper::new();
        let search_engine = SearchEngine::new();

        // Prepare Output
        let file = match OpenOptions::new().create(true).write(true).truncate(true).open(&output_path) {
            Ok(f) => f,
            Err(e) => {
                update_status("failed", "", Some(format!("Failed to open output file: {}", e)), None);
                return;
            }
        };

        let mut csv_writer = csv::WriterBuilder::new().from_writer(file);
        
        // Expanded Header
        let mut headers = vec![
            "company".to_string(), "country".to_string(), "website".to_string(), 
            "email".to_string(), "phone".to_string(), "source_page".to_string(), "status".to_string(), "timestamp".to_string()
        ];
        // Add columns for up to 5 contacts
        for i in 1..=5 {
            headers.push(format!("contact_{}_name", i));
            headers.push(format!("contact_{}_title", i));
            headers.push(format!("contact_{}_phone", i));
            headers.push(format!("contact_{}_email", i));
        }
        let _ = csv_writer.write_record(&headers);
        let _ = csv_writer.flush(); // Initial flush

        for (i, record) in records.iter().enumerate() {
            // Control Logic Loop
            loop {
                // Check for Stop/Pause
                let mut should_wait = false;
                {
                    let mut guard = jobs.lock().unwrap();
                    if let Some(job) = guard.get_mut(&job_id) {
                        if job.control_req == "stop" {
                            job.status = "stopped".to_string();
                            job.logs.push("Job stopped by user.".to_string());
                            return; // Exit thread
                        }
                        if job.control_req == "pause" {
                            job.status = "paused".to_string();
                            should_wait = true;
                        } else if job.status == "paused" && job.control_req == "none" {
                            // Was paused, now resumed
                            job.status = "processing".to_string();
                            job.logs.push("Job resumed.".to_string());
                        }
                    }
                }

                if should_wait {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    continue; // Re-check
                }
                break; // Proceed
            }

            {
                let mut guard = jobs.lock().unwrap();
                if let Some(job) = guard.get_mut(&job_id) {
                    job.processed_count = i + 1;
                    job.current_company = record.company.clone();
                }
            }

            // Processing logic (Copied/Adapted from main.rs V2)
            let mut target_url = record.website.clone();
            
            if target_url.is_none() || target_url.as_ref().unwrap().trim().is_empty() {
                update_status("", &record.company, Some(format!("Searching for {}...", record.company)), None);
                target_url = search_engine.search_company(&record.company, &record.country);
            }

            let mut status_str = "no_data";
            let mut final_url = String::new();
            let mut emails_str = String::new();
            let mut phones_str = String::new();
            let mut sources_str = String::new();
            let mut extracted_data = None;
            let mut contacts_vec = Vec::new();

            if let Some(url) = target_url {
                final_url = url.clone();
                update_status("", &record.company, Some(format!("Scraping {}", url)), None);
                
                let result = scraper_instance.scrape_site(&url);
                
                let emails_vec: Vec<String> = result.emails.into_iter().collect();
                let phones_vec: Vec<String> = result.phones.into_iter().collect();
                contacts_vec = result.contacts;

                emails_str = emails_vec.join("; ");
                phones_str = phones_vec.join("; ");
                sources_str = result.source_pages.join("; ");
                
                extracted_data = Some(ExtractedData {
                    emails: emails_vec,
                    phones: phones_vec,
                    contacts: contacts_vec.clone(),
                });

                status_str = match result.status {
                    ScrapeStatus::Success => "success",
                    ScrapeStatus::NoData => "no_data",
                    ScrapeStatus::Blocked => "blocked",
                    ScrapeStatus::Error => "error",
                };
            } else {
                status_str = "not_found";
                update_status("", &record.company, Some("Website not found".to_string()), None);
            }

            // Log success if data found
            if !emails_str.is_empty() || !phones_str.is_empty() {
                 update_status("", "", Some(format!("Found: {} | {}", emails_str, phones_str)), extracted_data);
            }

            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let mut record_row = vec![
                record.company.clone(),
                record.country.clone(),
                final_url,
                emails_str,
                phones_str,
                sources_str,
                status_str.to_string(),
                timestamp
            ];

            // Flatten Contacts (up to 5)
            for j in 0..5 {
                if let Some(contact) = contacts_vec.get(j) {
                    record_row.push(contact.name.clone().unwrap_or_default());
                    record_row.push(contact.title.clone().unwrap_or_default());
                    record_row.push(contact.phone.clone().unwrap_or_default());
                    record_row.push(contact.email.clone().unwrap_or_default());
                } else {
                    record_row.push("".to_string());
                    record_row.push("".to_string());
                    record_row.push("".to_string());
                    record_row.push("".to_string());
                }
            }

            let _ = csv_writer.write_record(&record_row);
            let _ = csv_writer.flush(); // FLUSH AFTER EVERY RECORD for partial download

            // Delay if not last
            if i < records.len() - 1 {
                 // Sleep inside thread, checking for stop every second?
                 // No, standard delay is fine, we check stop at top of loop.
                 // But for responsiveness, maybe we should break up the delay?
                 // Let's just use the standard delay for now to be safe.
                 business_scraper_lib::delay_manager::random_site_delay();   
            }
        }

        update_status("completed", "Done", Some("All records processed.".to_string()), None);
    }
}
