use std::process::ExitCode;

fn main() -> ExitCode {
    match torrentleech_cli::app::run_from_env() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            error.process_exit_code()
        }
    }
}
