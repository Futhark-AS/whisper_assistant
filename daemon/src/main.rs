fn main() {
    if let Err(error) = quedo_daemon::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
