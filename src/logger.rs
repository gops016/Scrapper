use log::LevelFilter;
use env_logger::Builder;
use std::io::Write;
use chrono::Local;

pub fn init() {
    Builder::new()
        .format(|buf, record| {
            writeln!(buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();
    
    log::info!("Logger initialized.");
}
