use std::time::Duration;
use std::thread;
use rand::Rng;
use log::info;

pub fn random_page_delay() {
    let mut rng = rand::thread_rng();
    let delay_secs = rng.gen_range(8..=30);
    info!("Waiting for {} seconds (Page Delay)...", delay_secs);
    thread::sleep(Duration::from_secs(delay_secs));
}

pub fn random_site_delay() {
    let mut rng = rand::thread_rng();
    let delay_secs = rng.gen_range(16..=45);
    info!("Waiting for {} seconds (Site Delay)...", delay_secs);
    thread::sleep(Duration::from_secs(delay_secs));
}
