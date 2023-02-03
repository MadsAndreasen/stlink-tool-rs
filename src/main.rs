use std::path::PathBuf;

use clap::Parser;

/// A tool to flash chinese ST-link dongles
/// Application is started when called without argument or after firmware load
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Probe the ST-Link adapter
    #[clap(short, long)]
    probe: Option<bool>,

    file: Option<PathBuf>,

}

fn main() {
    let args = Args::parse();

    println!("Hello, world!");
}
