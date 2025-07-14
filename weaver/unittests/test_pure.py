from weaver import pure


def test_hello() -> None:
    assert pure.hello() == "hello from Python"
