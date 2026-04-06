//! Python fixtures for `observe get-card` snapshots.

use super::CardFixtureCase;

/// Python `observe get-card` fixtures spanning functions, classes, imports,
/// decorators, and module-level constructs.
pub const PYTHON_CASES: [CardFixtureCase; 20] = [
    CardFixtureCase {
        name: "python_simple_function",
        file_name: "simple.py",
        source: "def greet(name: str) -> str:\n    return f\"hello {name}\"\n",
        line: 1,
        column: 5,
    },
    CardFixtureCase {
        name: "python_default_params",
        file_name: "default_params.py",
        source: "def greet(name: str = \"world\") -> str:\n    return name.title()\n",
        line: 1,
        column: 5,
    },
    CardFixtureCase {
        name: "python_varargs_kwargs",
        file_name: "varargs.py",
        source: "def collect(*items: str, **meta: str) -> tuple[int, int]:\n    return \
                 len(items), len(meta)\n",
        line: 1,
        column: 5,
    },
    CardFixtureCase {
        name: "python_async_function",
        file_name: "async_function.py",
        source: "async def fetch() -> str:\n    return \"ok\"\n",
        line: 1,
        column: 11,
    },
    CardFixtureCase {
        name: "python_class_init_methods",
        file_name: "class_init.py",
        source: "class Greeter:\n    def __init__(self, prefix: str) -> None:\n        \
                 self.prefix = prefix\n\n    def greet(self, name: str) -> str:\n        return \
                 f\"{self.prefix} {name}\"\n",
        line: 1,
        column: 7,
    },
    CardFixtureCase {
        name: "python_classmethod_staticmethod",
        file_name: "class_methods.py",
        source: "class Factory:\n    @classmethod\n    def build(cls) -> \"Factory\":\n        \
                 return cls()\n\n    @staticmethod\n    def version() -> str:\n        return \
                 \"1.0\"\n",
        line: 1,
        column: 7,
    },
    CardFixtureCase {
        name: "python_property_decorator",
        file_name: "property.py",
        source: "class User:\n    def __init__(self, name: str) -> None:\n        self._name = \
                 name\n\n    @property\n    def name(self) -> str:\n        return self._name\n",
        line: 1,
        column: 7,
    },
    CardFixtureCase {
        name: "python_nested_function",
        file_name: "nested.py",
        source: "def outer(value: int) -> int:\n    def inner(delta: int) -> int:\n        return \
                 value + delta\n\n    return inner(1)\n",
        line: 1,
        column: 5,
    },
    CardFixtureCase {
        name: "python_lambda_assignment",
        file_name: "lambda_assignment.py",
        source: "double = lambda value: value * 2\n",
        line: 1,
        column: 1,
    },
    CardFixtureCase {
        name: "python_generator_function",
        file_name: "generator.py",
        source: "def numbers() -> int:\n    yield 1\n    yield 2\n",
        line: 1,
        column: 5,
    },
    CardFixtureCase {
        name: "python_module_variable",
        file_name: "module_variable.py",
        source: "API_VERSION = \"v1\"\n",
        line: 1,
        column: 1,
    },
    CardFixtureCase {
        name: "python_import_block",
        file_name: "imports.py",
        source: "import os\nfrom pathlib import Path\n\nROOT = Path(os.getcwd())\n",
        line: 1,
        column: 1,
    },
    CardFixtureCase {
        name: "python_google_docstring",
        file_name: "google_docstring.py",
        source: "def parse(text: str) -> list[str]:\n    \"\"\"Parse words.\n\n    Args:\n        \
                 text: Input text.\n    \"\"\"\n    return text.split()\n",
        line: 1,
        column: 5,
    },
    CardFixtureCase {
        name: "python_numpy_docstring",
        file_name: "numpy_docstring.py",
        source:
            "def normalise(values: list[int]) -> list[int]:\n    \"\"\"Normalise values.\n\n    \
             Parameters\n    ----------\n    values:\n        Numbers to scale.\n    \"\"\"\n    \
             return values\n",
        line: 1,
        column: 5,
    },
    CardFixtureCase {
        name: "python_dataclass",
        file_name: "dataclass_case.py",
        source: "from dataclasses import dataclass\n\n@dataclass\nclass Point:\n    x: int\n    \
                 y: int\n",
        line: 4,
        column: 7,
    },
    CardFixtureCase {
        name: "python_abstract_base_class",
        file_name: "abc_case.py",
        source:
            "from abc import ABC, abstractmethod\n\nclass Loader(ABC):\n    @abstractmethod\n    \
             def load(self) -> str:\n        raise NotImplementedError\n",
        line: 3,
        column: 7,
    },
    CardFixtureCase {
        name: "python_complex_types",
        file_name: "complex_types.py",
        source: "from typing import Dict, Optional, Union\n\ndef project(values: Dict[str, \
                 Union[int, str]], key: Optional[str]) -> str:\n    return str(values.get(key or \
                 \"id\", \"0\"))\n",
        line: 3,
        column: 5,
    },
    CardFixtureCase {
        name: "python_decorator_stack",
        file_name: "decorator_stack.py",
        source: "def trace(func):\n    return func\n\n@trace\n@trace\ndef run() -> None:\n    \
                 pass\n",
        line: 6,
        column: 5,
    },
    CardFixtureCase {
        name: "python_control_flow",
        file_name: "control_flow.py",
        source:
            "def classify(value: int) -> str:\n    if value > 10:\n        return \"big\"\n    for \
             item in range(value):\n        if item == 2:\n            break\n    return \
             \"small\"\n",
        line: 1,
        column: 5,
    },
    CardFixtureCase {
        name: "python_module_doc_and_imports",
        file_name: "module_doc.py",
        source: "\"\"\"Tools for cache snapshots.\"\"\"\n\nimport math\n",
        line: 1,
        column: 1,
    },
];
