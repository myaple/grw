use color_eyre::eyre::Result;
use log::LevelFilter;
use std::env;
use std::fs;
use std::path::PathBuf;

pub fn init_logging(debug: bool) -> Result<()> {
    let log_level = if debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    let log_dir = get_default_log_dir()?;
    fs::create_dir_all(&log_dir)?;

    let log_file = log_dir.join("grw.log");

    env_logger::Builder::new()
        .filter_level(log_level)
        .target(env_logger::Target::Pipe(Box::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)?,
        )))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "{} [{}] - {}: {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();

    log::info!("Logging initialized with level: {log_level}");
    Ok(())
}

fn get_default_log_dir() -> Result<PathBuf> {
    if let Some(home) = env::var_os("HOME") {
        let xdg_state = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join(".local/state"));

        Ok(xdg_state.join("grw"))
    } else {
        Ok(PathBuf::from("/tmp/grw"))
    }
}
