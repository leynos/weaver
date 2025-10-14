use std::process::ExitCode;

fn main() -> ExitCode {
    match weaver_config::Config::load() {
        Ok(config) => {
            if let Err(error) = config.daemon_socket().prepare_filesystem() {
                eprintln!("Failed to prepare daemon socket directory: {error}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to load configuration: {error}");
            ExitCode::FAILURE
        }
    }
}
