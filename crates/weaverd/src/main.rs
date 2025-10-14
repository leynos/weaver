use std::process::ExitCode;

fn main() -> ExitCode {
    match weaver_config::Config::load() {
        Ok(config) => {
            let _ = config;
            ExitCode::SUCCESS
        }
        Err(_) => ExitCode::FAILURE,
    }
}
