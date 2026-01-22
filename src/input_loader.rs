use std::error::Error;
use std::fs::File;
use std::path::Path;
use log::{info, error};
use serde::Deserialize;
use calamine::{Reader, Xlsx, open_workbook, DataType};

#[derive(Debug, Deserialize, Clone)]
pub struct InputRecord {
    #[serde(rename = "Company", alias = "company", alias = "Company Name", alias = "company name", alias = "Business Name")]
    pub company: String,
    #[serde(rename = "Website", alias = "website", alias = "url", alias = "URL")]
    pub website: Option<String>,
    #[serde(rename = "Country", alias = "country", alias = "Location")]
    pub country: String,
}

pub fn load_records<P: AsRef<Path>>(filename: P) -> Vec<InputRecord> {
    let mut records = Vec::new();
    let path_ref = filename.as_ref();
    
    // Check if file exists
    if !path_ref.exists() {
         error!("Input file {:?} does not exist.", path_ref);
         return records;
    }

    // Attempt to detect if it is Excel based on extension or content
    // Simple check: Extension
    let is_excel = path_ref.extension().map_or(false, |ext| ext == "xlsx" || ext == "xls");

    if is_excel {
        return load_excel(path_ref);
    }
    
    // Default to CSV
    load_csv(path_ref)
}

fn load_csv(path: &Path) -> Vec<InputRecord> {
    let mut records = Vec::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            error!("Could not open CSV file: {}", e);
            return records;
        }
    };

    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    for result in rdr.deserialize() {
        match result {
            Ok(record) => {
                records.push(record);
            }
            Err(e) => {
                // If it fails, it might be because the user uploaded an Excel file named .csv
                // We could try to fallback to Excel reader if CSV fails significantly?
                // For now just log error.
                error!("Error parsing CSV record: {}", e);
            }
        }
    }
    info!("Loaded {} records from CSV {:?}", records.len(), path);
    records
}

fn load_excel(path: &Path) -> Vec<InputRecord> {
    let mut records = Vec::new();
    let mut excel: Xlsx<_> = match open_workbook(path) {
        Ok(wb) => wb,
        Err(e) => {
            error!("Could not open Excel file: {}", e);
            return records;
        }
    };

    // Calamine 0.24 usage
    let worksheets = excel.worksheets();
    if let Some((_name, range)) = worksheets.get(0) {
        // Assume first row is headers. 
        // We need to find indices for Company, Website, Country
        let mut company_idx = None;
        let mut website_idx = None;
        let mut country_idx = None;

        for (row_idx, row) in range.rows().enumerate() {
            if row_idx == 0 {
                // Header Row
                for (col_idx, cell) in row.iter().enumerate() {
                    let header = cell.to_string().to_lowercase();
                    if header.contains("company") || header.contains("business") { company_idx = Some(col_idx); }
                    else if header.contains("website") || header.contains("url") { website_idx = Some(col_idx); }
                    else if header.contains("country") || header.contains("location") { country_idx = Some(col_idx); }
                }
                
                if company_idx.is_none() {
                    error!("Excel Header missing 'Company' column");
                    return records;
                }
                continue;
            }

            // Data Rows
            let company = company_idx.and_then(|i| row.get(i)).map(|c| c.to_string()).unwrap_or_default();
            let website = website_idx.and_then(|i| row.get(i)).map(|c| c.to_string()).filter(|s| !s.is_empty());
            let country = country_idx.and_then(|i| row.get(i)).map(|c| c.to_string()).unwrap_or_default();

            if !company.is_empty() {
                records.push(InputRecord {
                    company,
                    website,
                    country
                });
            }
        }
    }
    
    info!("Loaded {} records from Excel {:?}", records.len(), path);
    records
}
