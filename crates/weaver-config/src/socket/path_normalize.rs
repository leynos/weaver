//! Lexical normalisation of socket directory paths.
//!
//! Canonicalisation resolves symlinks but reports the path with its `.` and
//! `..` components intact, so the traversal check in [`super::preparation`]
//! needs a folded form to compare against. Folding here is purely textual: it
//! never touches the filesystem, so the comparison stays independent of
//! whatever canonicalisation happens to find.

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};

/// Resolves `.` and `..` components lexically without touching the filesystem.
pub(super) fn normalize_parent_path(parent: &Utf8Path) -> Utf8PathBuf {
    let (components, is_absolute) = fold_components(parent);
    non_empty_path(assemble_path(is_absolute, components))
}

/// Folds the path's components into the reached names and absolute-ness.
fn fold_components(parent: &Utf8Path) -> (Vec<&str>, bool) {
    let mut components = Vec::new();
    let mut is_absolute = false;

    for component in parent.components() {
        fold_component(component, &mut components, &mut is_absolute);
    }

    (components, is_absolute)
}

/// Applies one component to the accumulated names.
fn fold_component<'a>(
    component: Utf8Component<'a>,
    components: &mut Vec<&'a str>,
    is_absolute: &mut bool,
) {
    match component {
        Utf8Component::RootDir => {
            *is_absolute = true;
            components.clear();
        }
        Utf8Component::CurDir | Utf8Component::Prefix(_) => {}
        Utf8Component::ParentDir => pop_or_keep_parent(components, *is_absolute),
        Utf8Component::Normal(normal_component) => components.push(normal_component),
    }
}

/// Pops a resolvable parent, or records `..` when the path is relative.
fn pop_or_keep_parent(components: &mut Vec<&str>, is_absolute: bool) {
    if parent_dir_should_pop(components.last().copied()) {
        components.pop();
    } else if !is_absolute {
        components.push("..");
    }
}

/// Rebuilds a path from the folded components.
fn assemble_path(is_absolute: bool, components: Vec<&str>) -> Utf8PathBuf {
    let mut normalized = if is_absolute {
        Utf8PathBuf::from("/")
    } else {
        Utf8PathBuf::new()
    };
    for component in components {
        normalized.push(component);
    }
    normalized
}

/// Reports the current directory rather than an empty path.
fn non_empty_path(normalized: Utf8PathBuf) -> Utf8PathBuf {
    if normalized.as_str().is_empty() {
        Utf8PathBuf::from(".")
    } else {
        normalized
    }
}

/// Reports whether a `..` component may pop the preceding component.
fn parent_dir_should_pop(last: Option<&str>) -> bool {
    matches!(last, Some(component) if component != "..")
}

#[cfg(test)]
mod tests {
    //! Unit tests for lexical path folding.

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("/var/lib/weaver/../run", "/var/lib/run")]
    #[case("/../etc", "/etc")]
    #[case("a/./b", "a/b")]
    #[case("a/../../b", "../b")]
    #[case("../a", "../a")]
    #[case("/", "/")]
    #[case(".", ".")]
    #[case("..", "..")]
    fn normalize_parent_path_folds_lexically(#[case] input: &str, #[case] expected: &str) {
        let normalized = normalize_parent_path(Utf8Path::new(input));

        assert_eq!(normalized, Utf8PathBuf::from(expected));
    }
}
