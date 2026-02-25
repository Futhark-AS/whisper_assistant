fn run_entry<F>(run_fn: F) -> i32
where
    F: FnOnce() -> quedo_daemon::error::AppResult<()>,
{
    match run_fn() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
    }
}

fn main() {
    std::process::exit(run_entry(quedo_daemon::run));
}

#[cfg(test)]
mod tests {
    use super::run_entry;
    use quedo_daemon::error::AppError;

    #[test]
    fn run_entry_returns_nonzero_on_fatal_error() {
        let code = run_entry(|| Err(AppError::Config("bad config".to_owned())));
        assert_eq!(code, 1);
    }

    #[test]
    fn run_entry_returns_zero_on_success() {
        let code = run_entry(|| Ok(()));
        assert_eq!(code, 0);
    }
}
