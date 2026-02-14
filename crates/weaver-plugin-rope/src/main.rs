//! Binary entrypoint for the rope actuator plugin.

use std::io::{self, BufReader, Write};

use weaver_plugin_rope::run;

fn main() {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    if let Err(error) = run(&mut reader, &mut writer) {
        writeln!(io::stderr().lock(), "{error}").ok();
        std::process::exit(1);
    }
}
