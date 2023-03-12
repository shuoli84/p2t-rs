use clap::Parser;
use poly2tri_rs::loader::{Loader, PlainFileLoader};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    path: std::path::PathBuf,
}

fn main() {
    let args = Args::parse();

    let mut file_loader = PlainFileLoader::default();
    let sweeper = file_loader
        .load(args.path.as_os_str().to_str().unwrap())
        .unwrap();

    let _result = sweeper.triangulate();
}
