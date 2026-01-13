#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# ///
"""Stage release artefacts according to a TOML configuration file.

This script reads a staging configuration, locates source artefacts,
copies them to a distribution directory, computes checksums, and
emits GitHub Actions outputs for downstream steps.
"""

from __future__ import annotations

import glob
import hashlib
import json
import os
import shutil
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import tomllib


@dataclass
class ArtefactSpec:
    """Specification for a single artefact to stage."""

    source: str
    required: bool = True
    output: str | None = None
    alternatives: list[str] = field(default_factory=list)


@dataclass
class TargetConfig:
    """Configuration for a specific build target."""

    platform: str
    arch: str
    target: str
    bin_ext: str = ""


@dataclass
class CommonConfig:
    """Common configuration shared across all targets."""

    dist_dir: str = "dist"
    checksum_algorithm: str = "sha256"
    staging_dir_template: str = "{bin_name}_{platform}_{arch}"
    artefacts: list[ArtefactSpec] = field(default_factory=list)


def load_config(config_path: Path) -> dict[str, Any]:
    """Load and parse the TOML configuration file."""
    with config_path.open("rb") as f:
        return tomllib.load(f)


def parse_artefact_spec(spec: dict[str, Any]) -> ArtefactSpec:
    """Parse a single artefact specification from config."""
    return ArtefactSpec(
        source=spec["source"],
        required=spec.get("required", True),
        output=spec.get("output"),
        alternatives=spec.get("alternatives", []),
    )


def parse_common_config(common: dict[str, Any]) -> CommonConfig:
    """Parse the [common] section of the configuration."""
    artefacts = [parse_artefact_spec(a) for a in common.get("artefacts", [])]
    return CommonConfig(
        dist_dir=common.get("dist_dir", "dist"),
        checksum_algorithm=common.get("checksum_algorithm", "sha256"),
        staging_dir_template=common.get(
            "staging_dir_template", "{bin_name}_{platform}_{arch}"
        ),
        artefacts=artefacts,
    )


def parse_target_config(target: dict[str, Any]) -> TargetConfig:
    """Parse a target-specific configuration section."""
    return TargetConfig(
        platform=target["platform"],
        arch=target["arch"],
        target=target["target"],
        bin_ext=target.get("bin_ext", ""),
    )


def find_source_file(pattern: str, alternatives: list[str]) -> Path | None:
    """Find a source file matching the pattern or alternatives."""
    matches = glob.glob(pattern, recursive=True)
    if matches:
        return Path(matches[0])

    for alt_pattern in alternatives:
        alt_matches = glob.glob(alt_pattern, recursive=True)
        if alt_matches:
            return Path(alt_matches[0])

    return None


def compute_checksum(file_path: Path, algorithm: str = "sha256") -> str:
    """Compute the checksum of a file."""
    hasher = hashlib.new(algorithm)
    with file_path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def expand_template(template: str, **kwargs: str) -> str:
    """Expand a template string with the given variables."""
    result = template
    for key, value in kwargs.items():
        result = result.replace(f"{{{key}}}", value)
    return result


def stage_artefacts(
    common: CommonConfig,
    target: TargetConfig,
    bin_name: str,
    workspace_root: Path,
) -> tuple[Path, dict[str, Path], dict[str, str], list[str]]:
    """Stage artefacts and return paths, checksums, and file list."""
    template_vars = {
        "bin_name": bin_name,
        "platform": target.platform,
        "arch": target.arch,
        "target": target.target,
        "bin_ext": target.bin_ext,
    }

    staging_dir_name = expand_template(common.staging_dir_template, **template_vars)
    dist_dir = workspace_root / common.dist_dir
    artifact_dir = dist_dir / staging_dir_name
    artifact_dir.mkdir(parents=True, exist_ok=True)

    artefact_map: dict[str, Path] = {}
    checksum_map: dict[str, str] = {}
    staged_files: list[str] = []

    for spec in common.artefacts:
        source_pattern = expand_template(spec.source, **template_vars)
        alternatives = [expand_template(a, **template_vars) for a in spec.alternatives]

        source_path = find_source_file(source_pattern, alternatives)

        if source_path is None:
            if spec.required:
                print(
                    f"::error::Required artefact not found: {source_pattern}",
                    file=sys.stderr,
                )
                sys.exit(1)
            continue

        dest_path = artifact_dir / source_path.name
        shutil.copy2(source_path, dest_path)

        checksum = compute_checksum(dest_path, common.checksum_algorithm)
        checksum_map[dest_path.name] = checksum
        staged_files.append(dest_path.name)

        if spec.output:
            artefact_map[spec.output] = dest_path

        # Write checksum file
        checksum_file = dest_path.with_suffix(
            dest_path.suffix + f".{common.checksum_algorithm}"
        )
        checksum_file.write_text(f"{checksum}  {dest_path.name}\n")
        staged_files.append(checksum_file.name)

    return artifact_dir, artefact_map, checksum_map, staged_files


def write_outputs(
    artifact_dir: Path,
    dist_dir: Path,
    artefact_map: dict[str, Path],
    checksum_map: dict[str, str],
    staged_files: list[str],
) -> None:
    """Write GitHub Actions outputs."""
    output_file = os.environ.get("GITHUB_OUTPUT")
    if not output_file:
        print("GITHUB_OUTPUT not set; printing outputs to stdout")
        print(f"artifact_dir={artifact_dir.absolute()}")
        print(f"dist_dir={dist_dir.absolute()}")
        print(f"staged_files={chr(10).join(staged_files)}")
        print(f"artefact_map={json.dumps({k: str(v) for k, v in artefact_map.items()})}")
        print(f"checksum_map={json.dumps(checksum_map)}")
        for key, path in artefact_map.items():
            print(f"{key}={path.absolute()}")
        return

    with open(output_file, "a") as f:
        f.write(f"artifact_dir={artifact_dir.absolute()}\n")
        f.write(f"dist_dir={dist_dir.absolute()}\n")

        # Multi-line output for staged_files
        f.write("staged_files<<EOF\n")
        f.write("\n".join(staged_files))
        f.write("\nEOF\n")

        f.write(
            f"artefact_map={json.dumps({k: str(v) for k, v in artefact_map.items()})}\n"
        )
        f.write(f"checksum_map={json.dumps(checksum_map)}\n")

        # Individual outputs for common artefact types
        for key, path in artefact_map.items():
            f.write(f"{key}={path.absolute()}\n")


def main() -> int:
    """Entry point for the staging script."""
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <config-file> <target>", file=sys.stderr)
        return 1

    config_path = Path(sys.argv[1])
    target_key = sys.argv[2]

    if not config_path.exists():
        print(f"Configuration file not found: {config_path}", file=sys.stderr)
        return 1

    config = load_config(config_path)

    common = parse_common_config(config.get("common", {}))
    targets = config.get("targets", {})

    if target_key not in targets:
        print(f"Target '{target_key}' not found in configuration", file=sys.stderr)
        print(f"Available targets: {', '.join(targets.keys())}", file=sys.stderr)
        return 1

    target = parse_target_config(targets[target_key])

    # Determine binary name from environment or default
    bin_name = os.environ.get("BIN_NAME", "weaver-cli")
    workspace_root = Path.cwd()

    artifact_dir, artefact_map, checksum_map, staged_files = stage_artefacts(
        common, target, bin_name, workspace_root
    )

    dist_dir = workspace_root / common.dist_dir
    write_outputs(artifact_dir, dist_dir, artefact_map, checksum_map, staged_files)

    print(f"Staged {len(staged_files)} files to {artifact_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
