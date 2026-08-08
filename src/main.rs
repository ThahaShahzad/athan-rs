mod audio;
mod autostart;
mod cli;
mod config;
mod display;
mod error;
mod hijri;
mod location;
mod scheduler;
mod tui;

fn main() {
    if let Err(err) = cli::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
