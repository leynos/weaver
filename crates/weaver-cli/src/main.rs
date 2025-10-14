use std::process::ExitCode;

fn main() -> ExitCode {
    match weaver_config::Config::load() {
        Ok(config) => {
            // The CLI will be extended to connect to `weaverd` using this
            // configuration in subsequent phases. For now we simply ensure the
            // configuration pipeline succeeds end-to-end.
            let _ = config;
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to load configuration: {error}");
            ExitCode::FAILURE
        }
    }
}
