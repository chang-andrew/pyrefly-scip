/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::path::Path;
use std::path::PathBuf;

use pretty_assertions::assert_eq;
use pyrefly_python::sys_info::PythonVersion;

use crate::report::scip::document::Indexer;
use crate::report::scip::document::relative_path;
use crate::report::scip::snapshot::render;
use crate::report::scip::symbols::Package;
use crate::state::require::Require;
use crate::test::util::mk_multi_file_state;

/// Renders the document of every file in `files`, in order, each under a `# <path>` header.
fn index(files: &[(&'static str, &str)], show_docs: bool) -> String {
    // The report is written whatever errors the files have, and some ported inputs have
    // some.
    let (handles, state) = mk_multi_file_state(files, Require::Everything, false);
    let transaction = state.transaction();
    let indexer = Indexer::new(
        &transaction,
        Package::new("snapshot-util".to_owned(), "0.1".to_owned()),
        PythonVersion::new(3, 12, 0),
        PathBuf::new(),
    );
    let mut out = String::new();
    for (name, source) in files {
        let handle = &handles[name];
        let path = relative_path(handle.path(), Path::new(""));
        let document = indexer.document(handle, path.clone());
        out.push_str(&format!("# {path}\n"));
        out.push_str(&render(source, &document, show_docs));
    }
    out
}

fn assert_index(files: &[(&'static str, &str)], expected: &str) {
    assert_eq!(expected.trim(), index(files, false).trim());
}

fn assert_index_with_docs(files: &[(&'static str, &str)], expected: &str) {
    assert_eq!(expected.trim(), index(files, true).trim());
}

#[test]
fn test_module_definition() {
    assert_index_with_docs(
        &[("pkg.mod", "\"\"\"Module docs.\"\"\"\n")],
        r#"
# pkg/mod.py
# < definition scip-python python snapshot-util 0.1 `pkg.mod`/__init__:
# kind Module display_name pkg.mod
# documentation (module) pkg.mod
# documentation Module docs.
"""Module docs."""
"#,
    );
}

#[test]
fn test_single_segment_module_is_not_escaped() {
    assert_index(
        &[("main", "x = 1\n")],
        r#"
# main.py
# < definition scip-python python snapshot-util 0.1 main/__init__:
x = 1
#^ definition  snapshot-util 0.1 main/x.
# enclosing [0, 0, 5]
"#,
    );
}

#[test]
fn test_definitions() {
    assert_index(
        &[(
            "defs",
            r#"
import functools

LIMIT: int = 3
counter = 0
counter += 1
first, _ = 1, 2

class Base:
    kind = "base"

    def __init__(self, name: str) -> None:
        self.name = name
        self.kind = name
        local = [n for n in name]

        def helper(value):
            return value

    @functools.cache
    def method(self, *args, **kwargs):
        return lambda x: x

    @staticmethod
    def static(x):
        x.attr = 1

    class Inner:
        pass

def outer(a, b=1):
    total = a
    total = b
    def inner():
        nonlocal total
        total = 2
    class Local:
        field = 1
        def method(self):
            self.value = 1
    return inner

def use_global():
    global counter
    counter = 2

for item in [1]:
    try:
        pass
    except Exception as error:
        pass

def generic[T](x: T) -> T:
    return x
"#,
        )],
        r#"
# defs.py
# < definition scip-python python snapshot-util 0.1 defs/__init__:

import functools
#      ^^^^^^^^^ reference[import]  python-stdlib 3.12 functools/__init__:

LIMIT: int = 3
#^^^^ definition  snapshot-util 0.1 defs/LIMIT.
# enclosing [3, 0, 14]
#      ^^^ reference  python-stdlib 3.12 builtins/int#
counter = 0
#^^^^^^ definition  snapshot-util 0.1 defs/counter.
# enclosing [4, 0, 11]
counter += 1
#^^^^^^ reference[write]  snapshot-util 0.1 defs/counter.
first, _ = 1, 2
#^^^^ definition  snapshot-util 0.1 defs/first.
# enclosing [6, 0, 15]

class Base:
#     ^^^^ definition  snapshot-util 0.1 defs/Base#
#      enclosing [8, 0, 28, 12]
    kind = "base"
#   ^^^^ definition  snapshot-util 0.1 defs/Base#kind.
#    enclosing [9, 4, 17]

    def __init__(self, name: str) -> None:
#       ^^^^^^^^ definition  snapshot-util 0.1 defs/Base#__init__().
#        enclosing [11, 4, 17, 24]
#                ^^^^ definition  snapshot-util 0.1 defs/Base#__init__().(self)
#                      ^^^^ definition  snapshot-util 0.1 defs/Base#__init__().(name)
#                            ^^^ reference  python-stdlib 3.12 builtins/str#
        self.name = name
#       ^^^^ reference  snapshot-util 0.1 defs/Base#__init__().(self)
#            ^^^^ definition  snapshot-util 0.1 defs/Base#name.
#             enclosing [12, 8, 24]
#                   ^^^^ reference  snapshot-util 0.1 defs/Base#__init__().(name)
        self.kind = name
#       ^^^^ reference  snapshot-util 0.1 defs/Base#__init__().(self)
#            ^^^^ reference[write]  snapshot-util 0.1 defs/Base#kind.
#                   ^^^^ reference  snapshot-util 0.1 defs/Base#__init__().(name)
        local = [n for n in name]
#       ^^^^^ definition local 0
#                ^ reference local 1
#                      ^ definition local 1
#                           ^^^^ reference  snapshot-util 0.1 defs/Base#__init__().(name)

        def helper(value):
#           ^^^^^^ definition  snapshot-util 0.1 defs/Base#helper().
#            enclosing [16, 8, 17, 24]
#                  ^^^^^ definition  snapshot-util 0.1 defs/Base#helper().(value)
            return value
#                  ^^^^^ reference  snapshot-util 0.1 defs/Base#helper().(value)

    @functools.cache
#    ^^^^^^^^^ reference  python-stdlib 3.12 functools/__init__:
#              ^^^^^ reference  python-stdlib 3.12 functools/cache().
    def method(self, *args, **kwargs):
#       ^^^^^^ definition  snapshot-util 0.1 defs/Base#method().
#        enclosing [20, 4, 21, 26]
#              ^^^^ definition  snapshot-util 0.1 defs/Base#method().(self)
#                     ^^^^ definition  snapshot-util 0.1 defs/Base#method().(args)
#                             ^^^^^^ definition  snapshot-util 0.1 defs/Base#method().(kwargs)
        return lambda x: x
#                     ^ definition local 2
#                        ^ reference local 2

    @staticmethod
#    ^^^^^^^^^^^^ reference  python-stdlib 3.12 builtins/staticmethod#
    def static(x):
#       ^^^^^^ definition  snapshot-util 0.1 defs/Base#static().
#        enclosing [24, 4, 25, 18]
#              ^ definition  snapshot-util 0.1 defs/Base#static().(x)
        x.attr = 1
#       ^ reference  snapshot-util 0.1 defs/Base#static().(x)

    class Inner:
#         ^^^^^ definition  snapshot-util 0.1 defs/Base#Inner#
#          enclosing [27, 4, 28, 12]
        pass

def outer(a, b=1):
#   ^^^^^ definition  snapshot-util 0.1 defs/outer().
#    enclosing [30, 0, 40, 16]
#         ^ definition  snapshot-util 0.1 defs/outer().(a)
#            ^ definition  snapshot-util 0.1 defs/outer().(b)
    total = a
#   ^^^^^ definition local 3
#           ^ reference  snapshot-util 0.1 defs/outer().(a)
    total = b
#   ^^^^^ reference[write] local 3
#           ^ reference  snapshot-util 0.1 defs/outer().(b)
    def inner():
#       ^^^^^ definition  snapshot-util 0.1 defs/outer().inner().
#        enclosing [33, 4, 35, 17]
        nonlocal total
#                ^^^^^ reference local 3
        total = 2
#       ^^^^^ reference[write] local 3
    class Local:
#         ^^^^^ definition  snapshot-util 0.1 defs/outer().Local#
#          enclosing [36, 4, 39, 26]
        field = 1
#       ^^^^^ definition  snapshot-util 0.1 defs/outer().Local#field.
#        enclosing [37, 8, 17]
        def method(self):
#           ^^^^^^ definition  snapshot-util 0.1 defs/outer().Local#method().
#            enclosing [38, 8, 39, 26]
#                  ^^^^ definition  snapshot-util 0.1 defs/outer().Local#method().(self)
            self.value = 1
#           ^^^^ reference  snapshot-util 0.1 defs/outer().Local#method().(self)
#                ^^^^^ definition  snapshot-util 0.1 defs/outer().Local#value.
#                 enclosing [39, 12, 26]
    return inner
#          ^^^^^ reference  snapshot-util 0.1 defs/outer().inner().

def use_global():
#   ^^^^^^^^^^ definition  snapshot-util 0.1 defs/use_global().
#    enclosing [42, 0, 44, 15]
    global counter
#          ^^^^^^^ reference  snapshot-util 0.1 defs/counter.
    counter = 2
#   ^^^^^^^ reference[write]  snapshot-util 0.1 defs/counter.

for item in [1]:
#   ^^^^ definition  snapshot-util 0.1 defs/item.
#    enclosing [46, 0, 50, 12]
    try:
        pass
    except Exception as error:
#          ^^^^^^^^^ reference  python-stdlib 3.12 builtins/Exception#
#                       ^^^^^ definition  snapshot-util 0.1 defs/error.
#                        enclosing [47, 4, 50, 12]
        pass

def generic[T](x: T) -> T:
#   ^^^^^^^ definition  snapshot-util 0.1 defs/generic().
#    enclosing [52, 0, 53, 12]
#           ^ definition  snapshot-util 0.1 defs/generic().[T]
#              ^ definition  snapshot-util 0.1 defs/generic().(x)
#                 ^ reference  snapshot-util 0.1 defs/generic().[T]
#                       ^ reference  snapshot-util 0.1 defs/generic().[T]
    return x
#          ^ reference  snapshot-util 0.1 defs/generic().(x)
"#,
    );
}

#[test]
fn test_symbol_documentation() {
    assert_index_with_docs(
        &[(
            "docs",
            r#"
import functools
from dataclasses import dataclass

class Base:
    """Base docs."""

    limit: int = 1

    def __init__(self, name: str, /, *, flag=False) -> None:
        self.name = name
        self.size: int = 0

    @functools.cache
    def method(self, a, *args, b: int = 2, **kwargs):
        """Method docs."""
        return [a]

@dataclass(frozen=True)
class Child(Base):
    tag = "child"

async def fetch() -> int:
    return 1

async def fetch_inferred():
    return "x"

VALUE = Child()
"#,
        )],
        r#"
# docs.py
# < definition scip-python python snapshot-util 0.1 docs/__init__:
# kind Module display_name docs
# documentation (module) docs

import functools
#      ^^^^^^^^^ reference[import]  python-stdlib 3.12 functools/__init__:
from dataclasses import dataclass
#    ^^^^^^^^^^^ reference[import]  python-stdlib 3.12 dataclasses/__init__:
#                       ^^^^^^^^^ reference[import]  python-stdlib 3.12 dataclasses/dataclass().

class Base:
#     ^^^^ definition  snapshot-util 0.1 docs/Base#
#      enclosing [4, 0, 16, 18]
#      kind Class display_name Base
#      signature class Base:
#      documentation ```python
#                    class Base:
#                    ```
#      documentation Base docs.
    """Base docs."""

    limit: int = 1
#   ^^^^^ definition  snapshot-util 0.1 docs/Base#limit.
#    enclosing [7, 4, 18]
#    kind Field display_name limit
#    signature limit: int
#    documentation ```python
#                  limit: int
#                  ```
#          ^^^ reference  python-stdlib 3.12 builtins/int#

    def __init__(self, name: str, /, *, flag=False) -> None:
#       ^^^^^^^^ definition  snapshot-util 0.1 docs/Base#__init__().
#        enclosing [9, 4, 11, 26]
#        kind Method display_name __init__
#        signature def __init__(
#                    self,
#                    name: str,
#                    /,
#                    *,
#                    flag=False
#                  ) -> None:
#        documentation ```python
#                      def __init__(
#                        self,
#                        name: str,
#                        /,
#                        *,
#                        flag=False
#                      ) -> None:
#                      ```
#                ^^^^ definition  snapshot-util 0.1 docs/Base#__init__().(self)
#                 kind Parameter display_name self
#                      ^^^^ definition  snapshot-util 0.1 docs/Base#__init__().(name)
#                       kind Parameter display_name name
#                            ^^^ reference  python-stdlib 3.12 builtins/str#
#                                       ^^^^ definition  snapshot-util 0.1 docs/Base#__init__().(flag)
#                                        kind Parameter display_name flag
        self.name = name
#       ^^^^ reference  snapshot-util 0.1 docs/Base#__init__().(self)
#            ^^^^ definition  snapshot-util 0.1 docs/Base#name.
#             enclosing [10, 8, 24]
#             kind Field display_name name
#             signature name
#             documentation ```python
#                           name
#                           ```
#                   ^^^^ reference  snapshot-util 0.1 docs/Base#__init__().(name)
        self.size: int = 0
#       ^^^^ reference  snapshot-util 0.1 docs/Base#__init__().(self)
#            ^^^^ definition  snapshot-util 0.1 docs/Base#size.
#             enclosing [11, 8, 26]
#             kind Field display_name size
#             signature size: int
#             documentation ```python
#                           size: int
#                           ```
#                  ^^^ reference  python-stdlib 3.12 builtins/int#

    @functools.cache
#    ^^^^^^^^^ reference  python-stdlib 3.12 functools/__init__:
#              ^^^^^ reference  python-stdlib 3.12 functools/cache().
    def method(self, a, *args, b: int = 2, **kwargs):
#       ^^^^^^ definition  snapshot-util 0.1 docs/Base#method().
#        enclosing [14, 4, 16, 18]
#        kind Method display_name method
#        signature @functools.cache
#                  def method(
#                    self,
#                    a,
#                    *args,
#                    b: int = 2,
#                    **kwargs
#                  ) -> list[Unknown]:
#        documentation ```python
#                      @functools.cache
#                      def method(
#                        self,
#                        a,
#                        *args,
#                        b: int = 2,
#                        **kwargs
#                      ) -> list[Unknown]:
#                      ```
#        documentation Method docs.
#              ^^^^ definition  snapshot-util 0.1 docs/Base#method().(self)
#               kind Parameter display_name self
#                    ^ definition  snapshot-util 0.1 docs/Base#method().(a)
#                     kind Parameter display_name a
#                        ^^^^ definition  snapshot-util 0.1 docs/Base#method().(args)
#                         kind Parameter display_name args
#                              ^ definition  snapshot-util 0.1 docs/Base#method().(b)
#                               kind Parameter display_name b
#                                 ^^^ reference  python-stdlib 3.12 builtins/int#
#                                            ^^^^^^ definition  snapshot-util 0.1 docs/Base#method().(kwargs)
#                                             kind Parameter display_name kwargs
        """Method docs."""
        return [a]
#               ^ reference  snapshot-util 0.1 docs/Base#method().(a)

@dataclass(frozen=True)
#^^^^^^^^^ reference  python-stdlib 3.12 dataclasses/dataclass().
#          ^^^^^^ reference  python-stdlib 3.12 dataclasses/dataclass().(frozen)
class Child(Base):
#     ^^^^^ definition  snapshot-util 0.1 docs/Child#
#      enclosing [19, 0, 20, 17]
#      relationship implementation  snapshot-util 0.1 docs/Base#
#      kind Class display_name Child
#      signature @dataclass(frozen=True)
#                class Child(Base):
#      documentation ```python
#                    @dataclass(frozen=True)
#                    class Child(Base):
#                    ```
#           ^^^^ reference  snapshot-util 0.1 docs/Base#
    tag = "child"
#   ^^^ definition  snapshot-util 0.1 docs/Child#tag.
#    enclosing [20, 4, 17]
#    kind Field display_name tag
#    signature tag: Literal['child']
#    documentation ```python
#                  tag: Literal['child']
#                  ```

async def fetch() -> int:
#         ^^^^^ definition  snapshot-util 0.1 docs/fetch().
#          enclosing [22, 0, 23, 12]
#          kind Function display_name fetch
#          signature async def fetch() -> int:
#          documentation ```python
#                        async def fetch() -> int:
#                        ```
#                    ^^^ reference  python-stdlib 3.12 builtins/int#
    return 1

async def fetch_inferred():
#         ^^^^^^^^^^^^^^ definition  snapshot-util 0.1 docs/fetch_inferred().
#          enclosing [25, 0, 26, 14]
#          kind Function display_name fetch_inferred
#          signature async def fetch_inferred() -> Literal['x']:
#          documentation ```python
#                        async def fetch_inferred() -> Literal['x']:
#                        ```
    return "x"

VALUE = Child()
#^^^^ definition  snapshot-util 0.1 docs/VALUE.
# enclosing [28, 0, 15]
# kind Variable display_name VALUE
# signature VALUE: Child
# documentation ```python
#               VALUE: Child
#               ```
#       ^^^^^ reference  snapshot-util 0.1 docs/Child#
"#,
    );
}

#[test]
fn test_keyword_arguments() {
    assert_index(
        &[(
            "kwargs",
            r#"
class Widget:
    def __init__(self, size: int, **kwargs) -> None:
        pass

def make(color: str, **kwargs) -> None:
    pass

Widget(size=1, extra=2)
make(color="red", extra=2)
"#,
        )],
        r#"
# kwargs.py
# < definition scip-python python snapshot-util 0.1 kwargs/__init__:

class Widget:
#     ^^^^^^ definition  snapshot-util 0.1 kwargs/Widget#
#      enclosing [1, 0, 3, 12]
    def __init__(self, size: int, **kwargs) -> None:
#       ^^^^^^^^ definition  snapshot-util 0.1 kwargs/Widget#__init__().
#        enclosing [2, 4, 3, 12]
#                ^^^^ definition  snapshot-util 0.1 kwargs/Widget#__init__().(self)
#                      ^^^^ definition  snapshot-util 0.1 kwargs/Widget#__init__().(size)
#                            ^^^ reference  python-stdlib 3.12 builtins/int#
#                                   ^^^^^^ definition  snapshot-util 0.1 kwargs/Widget#__init__().(kwargs)
        pass

def make(color: str, **kwargs) -> None:
#   ^^^^ definition  snapshot-util 0.1 kwargs/make().
#    enclosing [5, 0, 6, 8]
#        ^^^^^ definition  snapshot-util 0.1 kwargs/make().(color)
#               ^^^ reference  python-stdlib 3.12 builtins/str#
#                      ^^^^^^ definition  snapshot-util 0.1 kwargs/make().(kwargs)
    pass

Widget(size=1, extra=2)
#^^^^^ reference  snapshot-util 0.1 kwargs/Widget#
#      ^^^^ reference  snapshot-util 0.1 kwargs/Widget#__init__().(size)
make(color="red", extra=2)
#^^^ reference  snapshot-util 0.1 kwargs/make().
#    ^^^^^ reference  snapshot-util 0.1 kwargs/make().(color)
"#,
    );
}

/// A keyword argument to a class that does not declare it refers to the inherited key of a
/// TypedDict, or to the parameter of the `__new__` that the class inherits or defines.
#[test]
fn test_inherited_keyword_arguments() {
    assert_index(
        &[(
            "kwargs",
            r#"
from typing import TypedDict

class Base(TypedDict):
    clinic: int

class Child(Base):
    client: int

class Counter:
    def __new__(cls, start: int = 0) -> "Counter": ...

class SubCounter(Counter): ...

Child(clinic=1, client=2)
SubCounter(start=1)
enumerate([], start=1)
"#,
        )],
        r#"
# kwargs.py
# < definition scip-python python snapshot-util 0.1 kwargs/__init__:

from typing import TypedDict
#    ^^^^^^ reference[import]  python-stdlib 3.12 typing/__init__:
#                  ^^^^^^^^^ reference[import]  python-stdlib 3.12 typing/TypedDict.

class Base(TypedDict):
#     ^^^^ definition  snapshot-util 0.1 kwargs/Base#
#      enclosing [3, 0, 4, 15]
#      relationship implementation  python-stdlib 3.12 `_typeshed._type_checker_internals`/TypedDictFallback#
#          ^^^^^^^^^ reference  python-stdlib 3.12 typing/TypedDict.
    clinic: int
#   ^^^^^^ definition  snapshot-util 0.1 kwargs/Base#clinic.
#    enclosing [4, 4, 15]
#           ^^^ reference  python-stdlib 3.12 builtins/int#

class Child(Base):
#     ^^^^^ definition  snapshot-util 0.1 kwargs/Child#
#      enclosing [6, 0, 7, 15]
#      relationship implementation  snapshot-util 0.1 kwargs/Base#
#           ^^^^ reference  snapshot-util 0.1 kwargs/Base#
    client: int
#   ^^^^^^ definition  snapshot-util 0.1 kwargs/Child#client.
#    enclosing [7, 4, 15]
#           ^^^ reference  python-stdlib 3.12 builtins/int#

class Counter:
#     ^^^^^^^ definition  snapshot-util 0.1 kwargs/Counter#
#      enclosing [9, 0, 10, 54]
    def __new__(cls, start: int = 0) -> "Counter": ...
#       ^^^^^^^ definition  snapshot-util 0.1 kwargs/Counter#__new__().
#        enclosing [10, 4, 54]
#               ^^^ definition  snapshot-util 0.1 kwargs/Counter#__new__().(cls)
#                    ^^^^^ definition  snapshot-util 0.1 kwargs/Counter#__new__().(start)
#                           ^^^ reference  python-stdlib 3.12 builtins/int#
#                                        ^^^^^^^ reference  snapshot-util 0.1 kwargs/Counter#

class SubCounter(Counter): ...
#     ^^^^^^^^^^ definition  snapshot-util 0.1 kwargs/SubCounter#
#      enclosing [12, 0, 30]
#      relationship implementation  snapshot-util 0.1 kwargs/Counter#
#                ^^^^^^^ reference  snapshot-util 0.1 kwargs/Counter#

Child(clinic=1, client=2)
#^^^^ reference  snapshot-util 0.1 kwargs/Child#
#     ^^^^^^ reference  snapshot-util 0.1 kwargs/Base#clinic.
#               ^^^^^^ reference  snapshot-util 0.1 kwargs/Child#client.
SubCounter(start=1)
#^^^^^^^^^ reference  snapshot-util 0.1 kwargs/SubCounter#
#          ^^^^^ reference  snapshot-util 0.1 kwargs/Counter#__new__().(start)
enumerate([], start=1)
#^^^^^^^^ reference  python-stdlib 3.12 builtins/enumerate#
#             ^^^^^ reference  python-stdlib 3.12 builtins/enumerate#__new__().(start)
"#,
    );
}

// `b` in `a.b` after `import a.b` refers to the submodule. scip-python instead gives the head
// `a`, or sometimes the whole dotted name, the symbol of the first `a.*` submodule that the
// file imports (`dotted-import-head` and `dotted-module-span` in scip-goal/ADJUDICATIONS.md).
#[test]
fn test_dotted_submodule_reference() {
    assert_index(
        &[
            ("core.validators", "class PostalCode: ...\n"),
            ("core", ""),
            (
                "main",
                r#"
import core.validators

x = core.validators.PostalCode
"#,
            ),
        ],
        r#"
# core/validators.py
# < definition scip-python python snapshot-util 0.1 `core.validators`/__init__:
class PostalCode: ...
#     ^^^^^^^^^^ definition  snapshot-util 0.1 `core.validators`/PostalCode#
#      enclosing [0, 0, 21]

# core.py
# < definition scip-python python snapshot-util 0.1 core/__init__:

# main.py
# < definition scip-python python snapshot-util 0.1 main/__init__:

import core.validators
#      ^^^^^^^^^^^^^^^ reference[import]  snapshot-util 0.1 `core.validators`/__init__:

x = core.validators.PostalCode
#^ definition  snapshot-util 0.1 main/x.
# enclosing [3, 0, 30]
#   ^^^^ reference  snapshot-util 0.1 core/__init__:
#        ^^^^^^^^^^ reference  snapshot-util 0.1 `core.validators`/__init__:
#                   ^^^^^^^^^^ reference  snapshot-util 0.1 `core.validators`/PostalCode#
"#,
    );
}

#[test]
fn test_module_and_lambda_references() {
    assert_index(
        &[
            ("pkg.sub", "VALUE = 1\n"),
            ("pkg", ""),
            (
                "main",
                r#"
import json
from pkg import sub
from typing import ClassVar

json.loads("1")
sub.VALUE
by_first = sorted([(1, 2)], key=lambda pair: pair[0])

class Base:
    def __init__(self, origin: int) -> None:
        pass

class Child(Base):
    registry: ClassVar["Child"]

Child(origin=1)
"#,
            ),
        ],
        r#"
# pkg/sub.py
# < definition scip-python python snapshot-util 0.1 `pkg.sub`/__init__:
VALUE = 1
#^^^^ definition  snapshot-util 0.1 `pkg.sub`/VALUE.
# enclosing [0, 0, 9]

# pkg.py
# < definition scip-python python snapshot-util 0.1 pkg/__init__:

# main.py
# < definition scip-python python snapshot-util 0.1 main/__init__:

import json
#      ^^^^ reference[import]  python-stdlib 3.12 json/__init__:
from pkg import sub
#    ^^^ reference[import]  snapshot-util 0.1 pkg/__init__:
#               ^^^ reference[import]  snapshot-util 0.1 `pkg.sub`/__init__:
from typing import ClassVar
#    ^^^^^^ reference[import]  python-stdlib 3.12 typing/__init__:
#                  ^^^^^^^^ reference[import]  python-stdlib 3.12 typing/ClassVar.

json.loads("1")
#^^^ reference  python-stdlib 3.12 json/__init__:
#    ^^^^^ reference  python-stdlib 3.12 json/loads().
sub.VALUE
#^^ reference  snapshot-util 0.1 `pkg.sub`/__init__:
#   ^^^^^ reference  snapshot-util 0.1 `pkg.sub`/VALUE.
by_first = sorted([(1, 2)], key=lambda pair: pair[0])
#^^^^^^^ definition  snapshot-util 0.1 main/by_first.
# enclosing [7, 0, 53]
#          ^^^^^^ reference  python-stdlib 3.12 builtins/sorted().
#                           ^^^ reference  python-stdlib 3.12 builtins/sorted().(key)
#                                      ^^^^ definition local 0
#                                            ^^^^ reference local 0

class Base:
#     ^^^^ definition  snapshot-util 0.1 main/Base#
#      enclosing [9, 0, 11, 12]
    def __init__(self, origin: int) -> None:
#       ^^^^^^^^ definition  snapshot-util 0.1 main/Base#__init__().
#        enclosing [10, 4, 11, 12]
#                ^^^^ definition  snapshot-util 0.1 main/Base#__init__().(self)
#                      ^^^^^^ definition  snapshot-util 0.1 main/Base#__init__().(origin)
#                              ^^^ reference  python-stdlib 3.12 builtins/int#
        pass

class Child(Base):
#     ^^^^^ definition  snapshot-util 0.1 main/Child#
#      enclosing [13, 0, 14, 31]
#      relationship implementation  snapshot-util 0.1 main/Base#
#           ^^^^ reference  snapshot-util 0.1 main/Base#
    registry: ClassVar["Child"]
#   ^^^^^^^^ definition  snapshot-util 0.1 main/Child#registry.
#    enclosing [14, 4, 31]
#             ^^^^^^^^ reference  python-stdlib 3.12 typing/ClassVar.
#                       ^^^^^ reference  snapshot-util 0.1 main/Child#

Child(origin=1)
#^^^^ reference  snapshot-util 0.1 main/Child#
#     ^^^^^^ reference  snapshot-util 0.1 main/Base#__init__().(origin)
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unique/inherits_class.py and grandparent_impl.py.
// The occurrences and relationships match scip-python's, except `NotImplemented`, which
// pyrefly's typeshed declares as a variable rather than a class (`stdlib-same-name` in
// scip-goal/ADJUDICATIONS.md).
#[test]
fn test_ported_inherits_class() {
    assert_index(
        &[
            (
                "inherits_class",
                r#"class A:
    def x(self) -> int:
        raise NotImplemented

    def matched_despite_different_type(self, x: int):
        pass

class B(A):
    def x(self) -> int:
        return 5

    def matched_despite_different_type(self, x: int, y: int):
        pass

    def unrelated(self):
        pass
"#,
            ),
            (
                "grandparent_impl",
                r#"class A:
    def grandparent(self) -> bool:
        return True

class B(A):
    ...

class C(B):
    def grandparent(self) -> bool:
        return False
"#,
            ),
        ],
        r#"
# inherits_class.py
# < definition scip-python python snapshot-util 0.1 inherits_class/__init__:
class A:
#     ^ definition  snapshot-util 0.1 inherits_class/A#
#      enclosing [0, 0, 5, 12]
    def x(self) -> int:
#       ^ definition  snapshot-util 0.1 inherits_class/A#x().
#        enclosing [1, 4, 2, 28]
#         ^^^^ definition  snapshot-util 0.1 inherits_class/A#x().(self)
#                  ^^^ reference  python-stdlib 3.12 builtins/int#
        raise NotImplemented
#             ^^^^^^^^^^^^^^ reference  python-stdlib 3.12 builtins/NotImplemented.

    def matched_despite_different_type(self, x: int):
#       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 inherits_class/A#matched_despite_different_type().
#        enclosing [4, 4, 5, 12]
#                                      ^^^^ definition  snapshot-util 0.1 inherits_class/A#matched_despite_different_type().(self)
#                                            ^ definition  snapshot-util 0.1 inherits_class/A#matched_despite_different_type().(x)
#                                               ^^^ reference  python-stdlib 3.12 builtins/int#
        pass

class B(A):
#     ^ definition  snapshot-util 0.1 inherits_class/B#
#      enclosing [7, 0, 15, 12]
#      relationship implementation  snapshot-util 0.1 inherits_class/A#
#       ^ reference  snapshot-util 0.1 inherits_class/A#
    def x(self) -> int:
#       ^ definition  snapshot-util 0.1 inherits_class/B#x().
#        enclosing [8, 4, 9, 16]
#        relationship implementation  snapshot-util 0.1 inherits_class/A#x().
#         ^^^^ definition  snapshot-util 0.1 inherits_class/B#x().(self)
#                  ^^^ reference  python-stdlib 3.12 builtins/int#
        return 5

    def matched_despite_different_type(self, x: int, y: int):
#       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 inherits_class/B#matched_despite_different_type().
#        enclosing [11, 4, 12, 12]
#        relationship implementation  snapshot-util 0.1 inherits_class/A#matched_despite_different_type().
#                                      ^^^^ definition  snapshot-util 0.1 inherits_class/B#matched_despite_different_type().(self)
#                                            ^ definition  snapshot-util 0.1 inherits_class/B#matched_despite_different_type().(x)
#                                               ^^^ reference  python-stdlib 3.12 builtins/int#
#                                                    ^ definition  snapshot-util 0.1 inherits_class/B#matched_despite_different_type().(y)
#                                                       ^^^ reference  python-stdlib 3.12 builtins/int#
        pass

    def unrelated(self):
#       ^^^^^^^^^ definition  snapshot-util 0.1 inherits_class/B#unrelated().
#        enclosing [14, 4, 15, 12]
#                 ^^^^ definition  snapshot-util 0.1 inherits_class/B#unrelated().(self)
        pass

# grandparent_impl.py
# < definition scip-python python snapshot-util 0.1 grandparent_impl/__init__:
class A:
#     ^ definition  snapshot-util 0.1 grandparent_impl/A#
#      enclosing [0, 0, 2, 19]
    def grandparent(self) -> bool:
#       ^^^^^^^^^^^ definition  snapshot-util 0.1 grandparent_impl/A#grandparent().
#        enclosing [1, 4, 2, 19]
#                   ^^^^ definition  snapshot-util 0.1 grandparent_impl/A#grandparent().(self)
#                            ^^^^ reference  python-stdlib 3.12 builtins/bool#
        return True

class B(A):
#     ^ definition  snapshot-util 0.1 grandparent_impl/B#
#      enclosing [4, 0, 5, 7]
#      relationship implementation  snapshot-util 0.1 grandparent_impl/A#
#       ^ reference  snapshot-util 0.1 grandparent_impl/A#
    ...

class C(B):
#     ^ definition  snapshot-util 0.1 grandparent_impl/C#
#      enclosing [7, 0, 9, 20]
#      relationship implementation  snapshot-util 0.1 grandparent_impl/B#
#       ^ reference  snapshot-util 0.1 grandparent_impl/B#
    def grandparent(self) -> bool:
#       ^^^^^^^^^^^ definition  snapshot-util 0.1 grandparent_impl/C#grandparent().
#        enclosing [8, 4, 9, 20]
#        relationship implementation  snapshot-util 0.1 grandparent_impl/A#grandparent().
#                   ^^^^ definition  snapshot-util 0.1 grandparent_impl/C#grandparent().(self)
#                            ^^^^ reference  python-stdlib 3.12 builtins/bool#
        return False
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unique/builtin_import_refs.py.
// Matches scip-python's output, except `typing.Any`, which pyrefly's typeshed declares as a
// class rather than a variable (`stdlib-same-name` in scip-goal/ADJUDICATIONS.md).
#[test]
fn test_ported_builtin_import_refs() {
    assert_index(
        &[(
            "builtin_import_refs",
            r#"from typing import Any

print(Any)
"#,
        )],
        r#"
# builtin_import_refs.py
# < definition scip-python python snapshot-util 0.1 builtin_import_refs/__init__:
from typing import Any
#    ^^^^^^ reference[import]  python-stdlib 3.12 typing/__init__:
#                  ^^^ reference[import]  python-stdlib 3.12 typing/Any#

print(Any)
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^^^ reference  python-stdlib 3.12 typing/Any#
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unique/multiinherits_test.py.
// The occurrences and relationships match scip-python's.
#[test]
fn test_ported_multiinherits_test() {
    assert_index(
        &[(
            "multiinherits_test",
            r#"class Left:
    def one(self) -> int:
        return 1

    def shared(self) -> bool:
        return False

class Right:
    def two(self):
        return 2

    def shared(self) -> bool:
        return False

class Multi(Left, Right):
    def one(self) -> int:
        return 1

    def two(self):
        return 2

    def three(self):
        return 3

    def shared(self) -> bool:
        return True
"#,
        )],
        r#"
# multiinherits_test.py
# < definition scip-python python snapshot-util 0.1 multiinherits_test/__init__:
class Left:
#     ^^^^ definition  snapshot-util 0.1 multiinherits_test/Left#
#      enclosing [0, 0, 5, 20]
    def one(self) -> int:
#       ^^^ definition  snapshot-util 0.1 multiinherits_test/Left#one().
#        enclosing [1, 4, 2, 16]
#           ^^^^ definition  snapshot-util 0.1 multiinherits_test/Left#one().(self)
#                    ^^^ reference  python-stdlib 3.12 builtins/int#
        return 1

    def shared(self) -> bool:
#       ^^^^^^ definition  snapshot-util 0.1 multiinherits_test/Left#shared().
#        enclosing [4, 4, 5, 20]
#              ^^^^ definition  snapshot-util 0.1 multiinherits_test/Left#shared().(self)
#                       ^^^^ reference  python-stdlib 3.12 builtins/bool#
        return False

class Right:
#     ^^^^^ definition  snapshot-util 0.1 multiinherits_test/Right#
#      enclosing [7, 0, 12, 20]
    def two(self):
#       ^^^ definition  snapshot-util 0.1 multiinherits_test/Right#two().
#        enclosing [8, 4, 9, 16]
#           ^^^^ definition  snapshot-util 0.1 multiinherits_test/Right#two().(self)
        return 2

    def shared(self) -> bool:
#       ^^^^^^ definition  snapshot-util 0.1 multiinherits_test/Right#shared().
#        enclosing [11, 4, 12, 20]
#              ^^^^ definition  snapshot-util 0.1 multiinherits_test/Right#shared().(self)
#                       ^^^^ reference  python-stdlib 3.12 builtins/bool#
        return False

class Multi(Left, Right):
#     ^^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#
#      enclosing [14, 0, 25, 19]
#      relationship implementation  snapshot-util 0.1 multiinherits_test/Left#
#      relationship implementation  snapshot-util 0.1 multiinherits_test/Right#
#           ^^^^ reference  snapshot-util 0.1 multiinherits_test/Left#
#                 ^^^^^ reference  snapshot-util 0.1 multiinherits_test/Right#
    def one(self) -> int:
#       ^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#one().
#        enclosing [15, 4, 16, 16]
#        relationship implementation  snapshot-util 0.1 multiinherits_test/Left#one().
#           ^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#one().(self)
#                    ^^^ reference  python-stdlib 3.12 builtins/int#
        return 1

    def two(self):
#       ^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#two().
#        enclosing [18, 4, 19, 16]
#        relationship implementation  snapshot-util 0.1 multiinherits_test/Right#two().
#           ^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#two().(self)
        return 2

    def three(self):
#       ^^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#three().
#        enclosing [21, 4, 22, 16]
#             ^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#three().(self)
        return 3

    def shared(self) -> bool:
#       ^^^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#shared().
#        enclosing [24, 4, 25, 19]
#        relationship implementation  snapshot-util 0.1 multiinherits_test/Left#shared().
#              ^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#shared().(self)
#                       ^^^^ reference  python-stdlib 3.12 builtins/bool#
        return True
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unique/property_access.py.
// Matches scip-python's output.
#[test]
fn test_ported_property_access() {
    assert_index(
        &[(
            "property_access",
            r#"from typing import Sequence

class PropertyClass:
    def __init__(self):
        pass

    @property
    def prop_ref(self):
        return 5


xs = [PropertyClass()]

def usage(xs: Sequence[PropertyClass]):
    def nested():
        for x in xs:
            print(x.prop_ref)
"#,
        )],
        r#"
# property_access.py
# < definition scip-python python snapshot-util 0.1 property_access/__init__:
from typing import Sequence
#    ^^^^^^ reference[import]  python-stdlib 3.12 typing/__init__:
#                  ^^^^^^^^ reference[import]  python-stdlib 3.12 typing/Sequence#

class PropertyClass:
#     ^^^^^^^^^^^^^ definition  snapshot-util 0.1 property_access/PropertyClass#
#      enclosing [2, 0, 8, 16]
    def __init__(self):
#       ^^^^^^^^ definition  snapshot-util 0.1 property_access/PropertyClass#__init__().
#        enclosing [3, 4, 4, 12]
#                ^^^^ definition  snapshot-util 0.1 property_access/PropertyClass#__init__().(self)
        pass

    @property
#    ^^^^^^^^ reference  python-stdlib 3.12 builtins/property#
    def prop_ref(self):
#       ^^^^^^^^ definition  snapshot-util 0.1 property_access/PropertyClass#prop_ref().
#        enclosing [7, 4, 8, 16]
#                ^^^^ definition  snapshot-util 0.1 property_access/PropertyClass#prop_ref().(self)
        return 5


xs = [PropertyClass()]
#^ definition  snapshot-util 0.1 property_access/xs.
# enclosing [11, 0, 22]
#     ^^^^^^^^^^^^^ reference  snapshot-util 0.1 property_access/PropertyClass#

def usage(xs: Sequence[PropertyClass]):
#   ^^^^^ definition  snapshot-util 0.1 property_access/usage().
#    enclosing [13, 0, 16, 29]
#         ^^ definition  snapshot-util 0.1 property_access/usage().(xs)
#             ^^^^^^^^ reference  python-stdlib 3.12 typing/Sequence#
#                      ^^^^^^^^^^^^^ reference  snapshot-util 0.1 property_access/PropertyClass#
    def nested():
#       ^^^^^^ definition  snapshot-util 0.1 property_access/usage().nested().
#        enclosing [14, 4, 16, 29]
        for x in xs:
#           ^ definition local 0
#                ^^ reference  snapshot-util 0.1 property_access/usage().(xs)
            print(x.prop_ref)
#           ^^^^^ reference  python-stdlib 3.12 builtins/print().
#                 ^ reference local 0
#                   ^^^^^^^^ reference  snapshot-util 0.1 property_access/PropertyClass#prop_ref().
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unique/vars_inside_scopes.py.
// Matches scip-python's output.
#[test]
fn test_ported_vars_inside_scopes() {
    assert_index(
        &[(
            "vars_inside_scopes",
            r#"from typing import List

class X:
    items: List[int]

    def my_func(self):
        for x in self.items:
            y = x + 1

        if 5 in self.items:
            z = "oh ya"
"#,
        )],
        r#"
# vars_inside_scopes.py
# < definition scip-python python snapshot-util 0.1 vars_inside_scopes/__init__:
from typing import List
#    ^^^^^^ reference[import]  python-stdlib 3.12 typing/__init__:
#                  ^^^^ reference[import]  python-stdlib 3.12 typing/List.

class X:
#     ^ definition  snapshot-util 0.1 vars_inside_scopes/X#
#      enclosing [2, 0, 10, 23]
    items: List[int]
#   ^^^^^ definition  snapshot-util 0.1 vars_inside_scopes/X#items.
#    enclosing [3, 4, 20]
#          ^^^^ reference  python-stdlib 3.12 typing/List.
#               ^^^ reference  python-stdlib 3.12 builtins/int#

    def my_func(self):
#       ^^^^^^^ definition  snapshot-util 0.1 vars_inside_scopes/X#my_func().
#        enclosing [5, 4, 10, 23]
#               ^^^^ definition  snapshot-util 0.1 vars_inside_scopes/X#my_func().(self)
        for x in self.items:
#           ^ definition local 0
#                ^^^^ reference  snapshot-util 0.1 vars_inside_scopes/X#my_func().(self)
#                     ^^^^^ reference  snapshot-util 0.1 vars_inside_scopes/X#items.
            y = x + 1
#           ^ definition local 1
#               ^ reference local 0

        if 5 in self.items:
#               ^^^^ reference  snapshot-util 0.1 vars_inside_scopes/X#my_func().(self)
#                    ^^^^^ reference  snapshot-util 0.1 vars_inside_scopes/X#items.
            z = "oh ya"
#           ^ definition local 2
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unique/walrus.py.
// Differs from scip-python's output where scip-python is wrong: it gives the builtins `len`,
// `input` and `any` called around a walrus local symbols without definitions, which also
// shifts its numbering of the other locals, and it names `any_n`, bound by a walrus in a
// generator expression at module level, `any_n.` without the module.
#[test]
fn test_ported_walrus() {
    assert_index(
        &[(
            "walrus",
            r#"import re

some_list = [1, 2, 3, 4, 5, 100, 1000]
pattern = "^[A-Za-z0-9_]*$"
text = "Some_name123"

# if statement
if (n := len(some_list)) > 10:
    print(f"List is too long with {n} elements.")

if (match := re.search(pattern, text)) is not None:
    print("Found match:", match.group(0))


# comprehensions
def show_some_comprehension():
    [print(x, root) for x in some_list if (x % 2 == 0) and (root := x**0.5) > 5]


# while loop
while (line := input("Enter text: ")) != "quit":
    print(f"You entered: {line}")


# if + comprehension
if any((any_n := num) < 0 for num in some_list):
    print(f"Negative number found: {any_n}")
"#,
        )],
        r#"
# walrus.py
# < definition scip-python python snapshot-util 0.1 walrus/__init__:
import re
#      ^^ reference[import]  python-stdlib 3.12 re/__init__:

some_list = [1, 2, 3, 4, 5, 100, 1000]
#^^^^^^^^ definition  snapshot-util 0.1 walrus/some_list.
# enclosing [2, 0, 38]
pattern = "^[A-Za-z0-9_]*$"
#^^^^^^ definition  snapshot-util 0.1 walrus/pattern.
# enclosing [3, 0, 27]
text = "Some_name123"
#^^^ definition  snapshot-util 0.1 walrus/text.
# enclosing [4, 0, 21]

# if statement
if (n := len(some_list)) > 10:
#   ^ definition  snapshot-util 0.1 walrus/n.
#    enclosing [7, 0, 8, 49]
#        ^^^ reference  python-stdlib 3.12 builtins/len().
#            ^^^^^^^^^ reference  snapshot-util 0.1 walrus/some_list.
    print(f"List is too long with {n} elements.")
#   ^^^^^ reference  python-stdlib 3.12 builtins/print().
#                                  ^ reference  snapshot-util 0.1 walrus/n.

if (match := re.search(pattern, text)) is not None:
#   ^^^^^ definition  snapshot-util 0.1 walrus/match.
#    enclosing [10, 0, 11, 41]
#            ^^ reference  python-stdlib 3.12 re/__init__:
#               ^^^^^^ reference  python-stdlib 3.12 re/search().
#                      ^^^^^^^ reference  snapshot-util 0.1 walrus/pattern.
#                               ^^^^ reference  snapshot-util 0.1 walrus/text.
    print("Found match:", match.group(0))
#   ^^^^^ reference  python-stdlib 3.12 builtins/print().
#                         ^^^^^ reference  snapshot-util 0.1 walrus/match.
#                               ^^^^^ reference  python-stdlib 3.12 re/Match#group().


# comprehensions
def show_some_comprehension():
#   ^^^^^^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 walrus/show_some_comprehension().
#    enclosing [15, 0, 16, 80]
    [print(x, root) for x in some_list if (x % 2 == 0) and (root := x**0.5) > 5]
#    ^^^^^ reference  python-stdlib 3.12 builtins/print().
#          ^ reference local 0
#             ^^^^ reference local 1
#                       ^ definition local 0
#                            ^^^^^^^^^ reference  snapshot-util 0.1 walrus/some_list.
#                                          ^ reference local 0
#                                                           ^^^^ definition local 1
#                                                                   ^ reference local 0


# while loop
while (line := input("Enter text: ")) != "quit":
#      ^^^^ definition  snapshot-util 0.1 walrus/line.
#       enclosing [20, 0, 21, 33]
#              ^^^^^ reference  python-stdlib 3.12 builtins/input().
    print(f"You entered: {line}")
#   ^^^^^ reference  python-stdlib 3.12 builtins/print().
#                         ^^^^ reference  snapshot-util 0.1 walrus/line.


# if + comprehension
if any((any_n := num) < 0 for num in some_list):
#  ^^^ reference  python-stdlib 3.12 builtins/any().
#       ^^^^^ definition  snapshot-util 0.1 walrus/any_n.
#        enclosing [25, 0, 26, 44]
#                ^^^ reference local 2
#                             ^^^ definition local 2
#                                    ^^^^^^^^^ reference  snapshot-util 0.1 walrus/some_list.
    print(f"Negative number found: {any_n}")
#   ^^^^^ reference  python-stdlib 3.12 builtins/print().
#                                   ^^^^^ reference  snapshot-util 0.1 walrus/any_n.
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unique/field_docstring.py.
// The occurrences match scip-python's. Its documentation is laid out differently; the report's
// layout is the one Narrate parses, which the bench's consumer check (C1) verifies.
#[test]
fn test_ported_field_docstring() {
    assert_index(
        &[(
            "field_docstring",
            r#"# format-options: showDocs

class ClassWithField:

    a: int
    """Hello world, this is a"""
"#,
        )],
        r#"
# field_docstring.py
# < definition scip-python python snapshot-util 0.1 field_docstring/__init__:
# format-options: showDocs

class ClassWithField:
#     ^^^^^^^^^^^^^^ definition  snapshot-util 0.1 field_docstring/ClassWithField#
#      enclosing [2, 0, 5, 32]

    a: int
#   ^ definition  snapshot-util 0.1 field_docstring/ClassWithField#a.
#    enclosing [4, 4, 10]
#      ^^^ reference  python-stdlib 3.12 builtins/int#
    """Hello world, this is a"""
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unique/inferred_field_docstring.py.
// The occurrences match scip-python's. Its documentation is laid out differently; the report's
// layout is the one Narrate parses, which the bench's consumer check (C1) verifies.
#[test]
fn test_ported_inferred_field_docstring() {
    assert_index(
        &[(
            "inferred_field_docstring",
            r#"# format-options: showDocs

class ClassWithInferredField:
    def __init__(self, b: int):
        self.b = b
"#,
        )],
        r#"
# inferred_field_docstring.py
# < definition scip-python python snapshot-util 0.1 inferred_field_docstring/__init__:
# format-options: showDocs

class ClassWithInferredField:
#     ^^^^^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#
#      enclosing [2, 0, 4, 18]
    def __init__(self, b: int):
#       ^^^^^^^^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().
#        enclosing [3, 4, 4, 18]
#                ^^^^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().(self)
#                      ^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().(b)
#                         ^^^ reference  python-stdlib 3.12 builtins/int#
        self.b = b
#       ^^^^ reference  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().(self)
#            ^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#b.
#             enclosing [4, 8, 18]
#                ^ reference  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().(b)
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unique/module_docstring.py.
// The occurrences match scip-python's. Its documentation is laid out differently; the report's
// layout is the one Narrate parses, which the bench's consumer check (C1) verifies.
#[test]
fn test_ported_module_docstring() {
    assert_index(
        &[(
            "module_docstring",
            r#"# format-options: showDocs

"""
This is a docstring for this module
"""

def x():
    return 1
"#,
        )],
        r#"
# module_docstring.py
# < definition scip-python python snapshot-util 0.1 module_docstring/__init__:
# format-options: showDocs

"""
This is a docstring for this module
"""

def x():
#   ^ definition  snapshot-util 0.1 module_docstring/x().
#    enclosing [6, 0, 7, 12]
    return 1
"#,
    );
}
