use business_scraper_lib::{input_loader, scraper, extractor, resume_manager, delay_manager, logger, search_engine};
use business_scraper_lib::{Scraper, ScrapeStatus, ProgressState};


use std::error::Error;
use std::fs::OpenOptions;
use std::path::Path;
use log::{info, warn, error};
use chrono::Local;
// use csv::Writer; - Removed unused import


use scraper::{Scraper, ScrapeStatus};
use resume_manager::ProgressState;

fn main() -> Result<(), Box<dyn Error>> {
    logger::init();
    info!("Starting Business Scraper V2...");

    // 1. Load Inputs (Try input.csv)
    // Note: User asked for "import csv or excel". We support CSV.
    let input_file = "input_test_search.csv";
    let records = input_loader::load_records(input_file);
    if records.is_empty() {
        error!("No records found in {}. Please ensure the file exists and has headers: Company, Website, Country", input_file);
        return Ok(());
    }

    // 2. Load Resume State
    let mut progress = ProgressState::load();

    // 3. Initialize Engines
    let scraper_instance = Scraper::new();
    let search_engine = search_engine::SearchEngine::new();

    // 4. Initialize CSV Writer
    let output_csv = "results_v2.csv";
    let file_exists = Path::new(output_csv).exists();
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(output_csv)?;

    let mut csv_writer = csv::WriterBuilder::new()
        .has_headers(!file_exists)
        .from_writer(file);

    if !file_exists {
        csv_writer.write_record(&["company", "country", "website", "email", "phone", "source_page", "status", "timestamp"])?;
        csv_writer.flush()?;
    }

    let total = records.len();
    let mut processed_count = 0;

    for (i, record) in records.iter().enumerate() {
        // ID for resume tracking: Company Name is best unique identifier
        let unique_id = record.company.trim().to_string();
        
        if progress.contains(&unique_id) {
            continue;
        }

        processed_count += 1;
        info!("Processing {} / {} : {} ({})", i + 1, total, record.company, record.country);

        // DELAY between items
        if processed_count > 1 {
            delay_manager::random_site_delay();
        }

        // Determine Website
        let mut target_url = record.website.clone();
        
        if target_url.is_none() || target_url.as_ref().unwrap().trim().is_empty() {
            info!("No website provided for '{}'. Searching...", record.company);
            target_url = search_engine.search_company(&record.company, &record.country);
        }

        let mut emails_str = String::new();
        let mut phones_str = String::new();
        let mut sources_str = String::new();
        let mut status_str = "no_data";
        let mut final_url = String::new();

        if let Some(url) = target_url {
            final_url = url.clone();
            // Scrape
            let result = scraper_instance.scrape_site(&url);
            
            emails_str = result.emails.into_iter().collect::<Vec<_>>().join("; ");
            phones_str = result.phones.into_iter().collect::<Vec<_>>().join("; ");
            sources_str = result.source_pages.join("; ");
            
            status_str = match result.status {
                ScrapeStatus::Success => "success",
                ScrapeStatus::NoData => "no_data",
                ScrapeStatus::Blocked => "blocked",
                ScrapeStatus::Error => "error",
            };
        } else {
            status_str = "not_found";
            warn!("Could not find website for {}", record.company);
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        if let Err(e) = csv_writer.write_record(&[
            &record.company,
            &record.country,
            &final_url,
            &emails_str,
            &phones_str,
            &sources_str,
            status_str,
            &timestamp
        ]) {
            error!("Failed to write CSV record for {}: {}", record.company, e);
        }
        csv_writer.flush()?;

        // Update Progress
        progress.mark_complete(unique_id);
    }

    info!("Scraping Completed. Processed {} new companies.", processed_count);
    Ok(())
}
