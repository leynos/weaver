//! Fallible observation protocol for the disposable macOS caller-boundary probe.

use std::{
    ffi::OsString,
    io::{self, BufRead as _, Read as _, Write as _},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail, ensure};
use cap_std::{ambient_authority, fs::Dir};
use weaver_sandbox::{Sandbox, SandboxCommand, SandboxProfile, process::Stdio};

const LIMIT: u64 = 32 * 1024;
const HOST_MARKER_HEX: &str = "6861726d6c6573732d686f73742d6d61726b6572";

struct Inputs {
    mode: OsString,
    secret: PathBuf,
    control: PathBuf,
    host: PathBuf,
    executable: PathBuf,
    grants: Vec<PathBuf>,
}

/// Dispatches one initial-thread caller or its target without a test runner.
pub(super) fn run() -> Result<()> {
    let inputs = inputs()?;
    if inputs.mode == "target" {
        target(&inputs)
    } else {
        ensure!(
            inputs.mode == "control" || inputs.mode == "activated",
            "unknown mode"
        );
        // No activation or descendant is possible until the controller proves
        // ownership of this still-unreaped session leader and admits it.
        let mut admission = [0_u8; 1];
        io::stdin().lock().read_exact(&mut admission)?;
        ensure!(admission == [b'A'], "invalid controller admission");
        caller(&inputs)
    }
}

fn inputs() -> Result<Inputs> {
    let mut arguments = std::env::args_os().skip(1);
    let mode = arguments.next().context("missing mode")?;
    let secret = PathBuf::from(arguments.next().context("missing secret")?);
    let control = PathBuf::from(arguments.next().context("missing control")?);
    let host = PathBuf::from(arguments.next().context("missing host directory")?);
    let executable = std::env::current_exe().context("current executable")?;
    let grants = arguments.map(PathBuf::from).collect();
    Ok(Inputs {
        mode,
        secret,
        control,
        host,
        executable,
        grants,
    })
}

fn canonical(path: &Path) -> Result<PathBuf> {
    let directory = Dir::open_ambient_dir("/", ambient_authority())?;
    let relative = path.strip_prefix("/").context("absolute path required")?;
    Ok(Path::new("/").join(directory.canonicalize(relative)?))
}

fn validate_grants(inputs: &Inputs) -> Result<()> {
    let secret = canonical(&inputs.secret)?;
    let control = canonical(&inputs.control)?;
    let host = canonical(&inputs.host)?;
    let fixture = secret
        .parent()
        .and_then(Path::parent)
        .context("fixture root")?;
    ensure!(
        secret == inputs.secret && control == inputs.control && host == inputs.host,
        "fixture paths must already be canonical"
    );
    ensure!(
        secret == fixture.join("denied/secret.txt"),
        "secret topology changed"
    );
    ensure!(
        control == fixture.join("allowed/control.txt"),
        "control topology changed"
    );
    ensure!(host == fixture.join("host-only"), "host topology changed");
    let expected_runtime = [canonical(Path::new("/usr/lib"))?];
    ensure!(
        inputs.grants == expected_runtime,
        "unreviewed runtime grants"
    );
    let executable = canonical(&inputs.executable)?;
    for grant in std::iter::once(&executable)
        .chain(std::iter::once(&control))
        .chain(&inputs.grants)
    {
        let resolved = canonical(grant)?;
        ensure!(&resolved == grant, "grant must be canonical");
        ensure!(
            !secret.starts_with(grant) && !host.starts_with(grant),
            "overlapping grant"
        );
        ensure!(
            !grant.starts_with(fixture) || grant == &control,
            "fixture grant too broad"
        );
        ensure!(grant != Path::new("/"), "root grant forbidden");
    }
    Ok(())
}

fn read_observation(label: &str, path: &Path) -> Result<()> {
    let parent = path.parent().context("read parent")?;
    let name = path.file_name().context("read file name")?;
    let directory = match Dir::open_ambient_dir(parent, ambient_authority()) {
        Ok(directory) => directory,
        Err(error) => return report_error(label, "open_parent", &error),
    };
    let mut file = match directory.open(name) {
        Ok(file) => file,
        Err(error) => return report_error(label, "open_file", &error),
    };
    let mut bytes = Vec::new();
    match (&mut file).take(LIMIT + 1).read_to_end(&mut bytes) {
        Ok(_) => {
            ensure!(
                u64::try_from(bytes.len())? <= LIMIT,
                "file observation too large"
            );
            let mut encoded = String::new();
            for byte in bytes {
                use std::fmt::Write as _;
                write!(&mut encoded, "{byte:02x}")?;
            }
            emit(&format!("OBS\t{label}\tok\t{encoded}"))
        }
        Err(error) => report_error(label, "read_file", &error),
    }
}

fn write_observation(label: &str, host: &Path) -> Result<()> {
    let directory = match Dir::open_ambient_dir(host, ambient_authority()) {
        Ok(directory) => directory,
        Err(error) => return report_error(label, "open_parent", &error),
    };
    match directory.write(label, b"harmless-host-marker") {
        Ok(()) => emit(&format!("OBS\t{label}\tok\t{HOST_MARKER_HEX}")),
        Err(error) => report_error(label, "write_file", &error),
    }
}

fn report_error(label: &str, stage: &str, error: &io::Error) -> Result<()> {
    emit(&format!(
        "OBS\t{label}\terr\t{stage}\t{:?}\t{}",
        error.kind(),
        error.raw_os_error().unwrap_or(-1)
    ))
}

fn emit(line: &str) -> Result<()> {
    let mut output = io::stdout().lock();
    writeln!(output, "{line}")?;
    output.flush()?;
    Ok(())
}

fn command(inputs: &Inputs) -> SandboxCommand {
    let mut child = SandboxCommand::new(&inputs.executable);
    child
        .arg("target")
        .arg(&inputs.secret)
        .arg(&inputs.control)
        .arg(&inputs.host);
    child
        .args(&inputs.grants)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    child
}

fn caller(inputs: &Inputs) -> Result<()> {
    validate_grants(inputs)?;
    read_observation("before_read", &inputs.secret)?;
    write_observation("before_write", &inputs.host)?;
    let mut profile = SandboxProfile::new()
        .allow_executable(&inputs.executable)
        .allow_read_path(&inputs.control)
        .allow_full_environment();
    for grant in &inputs.grants {
        profile = profile.allow_read_path(grant);
    }
    emit("EVENT\tspawn_start")?;
    // The controller owns this whole session, including any partly restricted
    // caller and target. Error paths deliberately leave cleanup to that owner.
    let mut child = if inputs.mode == "activated" {
        Sandbox::new(profile).spawn(command(inputs))?
    } else {
        command(inputs).spawn()?
    };
    let output = child.stdout.take().context("target stdout missing")?;
    let mut reader = io::BufReader::new(output).take(LIMIT + 1);
    let mut consumed = 0;
    loop {
        let mut line = String::new();
        let count = reader.read_line(&mut line)?;
        consumed += count;
        ensure!(
            count > 0 && u64::try_from(consumed)? <= LIMIT,
            "target protocol EOF/overflow"
        );
        if line == "READY\n" {
            emit("READY")?;
            break;
        }
        ensure!(line.starts_with("OBS\t"), "unexpected target protocol");
        emit(line.trim_end_matches('\n'))?;
    }
    read_observation("during_read", &inputs.secret)?;
    write_observation("during_write", &inputs.host)?;
    let mut input = child.stdin.take().context("target stdin missing")?;
    input.write_all(b"R")?;
    drop(input);
    let status = child.wait()?;
    ensure!(status.success(), "target unsuccessful: {status}");
    let mut trailing = Vec::new();
    reader.read_to_end(&mut trailing)?;
    ensure!(trailing.is_empty(), "trailing target output");
    emit("EVENT\ttarget_wait\t0")?;
    read_observation("after_read", &inputs.secret)?;
    write_observation("after_write", &inputs.host)?;
    emit("EVENT\tcomplete")
}

fn target(inputs: &Inputs) -> Result<()> {
    read_observation("target_control", &inputs.control)?;
    read_observation("target_secret", &inputs.secret)?;
    emit("READY")?;
    let mut release = [0_u8; 1];
    io::stdin().lock().read_exact(&mut release)?;
    if release != [b'R'] {
        bail!("unexpected release byte");
    }
    Ok(())
}
