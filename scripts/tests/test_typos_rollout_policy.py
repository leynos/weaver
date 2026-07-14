"""Characterize shared dictionary validation and merge policy."""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import pytest

from typos_rollout_test_support import dictionary_text

if typ.TYPE_CHECKING:
    import types

type RolloutModules = tuple[types.ModuleType, types.ModuleType, types.ModuleType]

# Keep the policy violation out of this test module's own spelling scan.
PROHIBITED_PHRASE = "hand" + "-written"


@pytest.mark.parametrize(
    ("document", "expected"),
    [
        pytest.param(
            dictionary_text().replace("schema = 1", "schema = 2"),
            (ValueError, "unsupported dictionary schema 2"),
            id="unsupported-schema",
        ),
        pytest.param(
            dictionary_text().replace(
                '[oxford]\nstems = ["organ"]',
                'oxford = "bad"',
            ),
            (TypeError, "'oxford' must be a table"),
            id="invalid-table",
        ),
        pytest.param(
            dictionary_text().replace('stems = ["organ"]', "stems = [1]"),
            (TypeError, "'stems' must be a list of strings"),
            id="invalid-string-list",
        ),
        pytest.param(
            dictionary_text().replace(
                "[words.corrections]",
                "[words.corrections]\nteh = 1",
            ),
            (TypeError, "word corrections must map strings to strings"),
            id="invalid-word-correction",
        ),
        pytest.param(
            dictionary_text().replace(
                "[phrases.corrections]",
                f"[phrases.corrections]\n'{PROHIBITED_PHRASE}' = 1",
            ),
            (TypeError, "phrase corrections must map strings to strings"),
            id="invalid-phrase-correction",
        ),
    ],
)
def test_dictionary_validation_rejects_invalid_documents(
    rollout_modules: RolloutModules,
    tmp_path: Path,
    document: str,
    expected: tuple[type[Exception], str],
) -> None:
    """Schema, table, string-list and correction types remain validated."""
    _, _, rollout = rollout_modules
    expected_error, expected_message = expected
    source = tmp_path / "base.toml"
    source.write_text(document, encoding="utf-8")

    with pytest.raises(expected_error, match=rf"^{re.escape(expected_message)}$"):
        rollout.load_dictionary(source)


def test_merge_rejects_conflicting_corrections(
    rollout_modules: RolloutModules,
) -> None:
    """A local overlay cannot silently weaken shared word or phrase policy."""
    _, _, rollout = rollout_modules
    base = rollout.Dictionary(
        corrections=(("teh", "the"),),
        phrase_corrections=((PROHIBITED_PHRASE, "handwritten"),),
    )

    merged = rollout.merge_dictionaries(base, rollout.Dictionary())

    assert merged.phrase_corrections == base.phrase_corrections, (
        "an empty local policy discarded the shared phrase correction"
    )
    with pytest.raises(
        ValueError,
        match=r"^conflicting correction for 'teh': 'the' != 'ten'$",
    ):
        rollout.merge_dictionaries(
            base,
            rollout.Dictionary(corrections=(("teh", "ten"),)),
        )
    with pytest.raises(
        ValueError,
        match=(
            rf"^conflicting phrase correction for '{re.escape(PROHIBITED_PHRASE)}': "
            r"'handwritten' != 'other'$"
        ),
    ):
        rollout.merge_dictionaries(
            base,
            rollout.Dictionary(phrase_corrections=((PROHIBITED_PHRASE, "other"),)),
        )


def test_merge_sorts_word_and_phrase_corrections(
    rollout_modules: RolloutModules,
) -> None:
    """Word and phrase policies retain deterministic key ordering."""
    _, _, rollout = rollout_modules
    base = rollout.Dictionary(
        corrections=(("zeta", "last"),),
        phrase_corrections=(("zeta phrase", "last phrase"),),
    )
    local = rollout.Dictionary(
        corrections=(("alpha", "first"),),
        phrase_corrections=(("alpha phrase", "first phrase"),),
    )

    merged = rollout.merge_dictionaries(base, local)

    assert merged.corrections == (("alpha", "first"), ("zeta", "last")), (
        "word correction ordering changed"
    )
    assert merged.phrase_corrections == (
        ("alpha phrase", "first phrase"),
        ("zeta phrase", "last phrase"),
    ), "phrase correction ordering changed"
