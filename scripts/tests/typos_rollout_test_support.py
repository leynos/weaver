"""Shared data builders for spelling rollout tests."""


def dictionary_text(stem: str = "organ") -> str:
    """Return a minimal valid shared-dictionary document.

    Parameters
    ----------
    stem
        Oxford stem to include in the shared dictionary.

    Returns
    -------
    str
        Minimal parseable shared-dictionary TOML.
    """
    return (
        'schema = 1\n\n[oxford]\nstems = ["'
        + stem
        + '"]\n\n[words]\naccepted = []\n\n[words.corrections]\n\n'
        + "[phrases.corrections]\n\n"
        + "[patterns]\nignore = []\n\n[files]\nexclude = []\n"
    )
