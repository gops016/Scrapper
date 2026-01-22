pub mod input_loader;
pub mod scraper;
pub mod extractor;
pub mod resume_manager;
pub mod delay_manager;
pub mod logger;
pub mod search_engine;

// Exporting types for convenience
pub use input_loader::InputRecord;
pub use scraper::{Scraper, ScrapeStatus};
pub use search_engine::SearchEngine;
pub use resume_manager::ProgressState;
pub use extractor::Extractor;
