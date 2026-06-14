//! Regenerate the `OrthoConfig` consumer boundary matrix.

use std::env;

use camino::Utf8Path;
use cap_std::{ambient_authority, fs::Dir};
use weaver_docs_gate::{load_manifest, render_matrix};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let manifest_path = args
        .next()
        .ok_or_else(|| "usage: render_boundary_matrix <manifest> <output>".to_owned())?;
    let output_path = args
        .next()
        .ok_or_else(|| "usage: render_boundary_matrix <manifest> <output>".to_owned())?;

    if args.next().is_some() {
        return Err("usage: render_boundary_matrix <manifest> <output>".to_owned());
    }

    let manifest =
        load_manifest(Utf8Path::new(&manifest_path)).map_err(|error| error.to_string())?;
    write_output(Utf8Path::new(&output_path), render_matrix(&manifest))
}

fn write_output(path: &Utf8Path, content: String) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid output path: {path}"))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("invalid output path: {path}"))?;
    let dir =
        Dir::open_ambient_dir(parent, ambient_authority()).map_err(|error| error.to_string())?;
    dir.write(file_name, content)
        .map_err(|error| error.to_string())
}
