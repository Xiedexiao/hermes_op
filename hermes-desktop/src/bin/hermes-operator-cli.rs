#[path = "../cli/mod.rs"]
mod cli;

fn main() {
    let mut stdout = std::io::stdout();

    if let Err(error) = cli::run(std::env::args_os(), &mut stdout) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
