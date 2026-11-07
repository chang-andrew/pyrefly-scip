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

/// The body of a `@no_type_check` function is indexed although it is not type checked. As in type
/// checking, its parameters' annotations are ignored, so `box` is `Any` and `size` on it has no
/// occurrence.
#[test]
fn test_no_type_check_body() {
    assert_index(
        &[(
            "main",
            r#"
from typing import no_type_check

LIMIT = 3

class Box:
    size = 1

@no_type_check
def run(box: Box, flag=True) -> int:
    total: int = "x"
    if flag:
        return LIMIT + box.size + total
    return LIMIT
"#,
        )],
        r#"
# main.py
# < definition scip-python python snapshot-util 0.1 main/__init__:

from typing import no_type_check
#    ^^^^^^ reference[import]  python-stdlib 3.12 typing/__init__:
#                  ^^^^^^^^^^^^^ reference[import]  python-stdlib 3.12 typing/no_type_check().

LIMIT = 3
#^^^^ definition  snapshot-util 0.1 main/LIMIT.
# enclosing [3, 0, 9]

class Box:
#     ^^^ definition  snapshot-util 0.1 main/Box#
#      enclosing [5, 0, 6, 12]
    size = 1
#   ^^^^ definition  snapshot-util 0.1 main/Box#size.
#    enclosing [6, 4, 12]

@no_type_check
#^^^^^^^^^^^^^ reference  python-stdlib 3.12 typing/no_type_check().
def run(box: Box, flag=True) -> int:
#   ^^^ definition  snapshot-util 0.1 main/run().
#    enclosing [9, 0, 13, 16]
#       ^^^ definition  snapshot-util 0.1 main/run().(box)
#            ^^^ reference  snapshot-util 0.1 main/Box#
#                 ^^^^ definition  snapshot-util 0.1 main/run().(flag)
#                               ^^^ reference  python-stdlib 3.12 builtins/int#
    total: int = "x"
#   ^^^^^ definition local 0
#          ^^^ reference  python-stdlib 3.12 builtins/int#
    if flag:
#      ^^^^ reference  snapshot-util 0.1 main/run().(flag)
        return LIMIT + box.size + total
#              ^^^^^ reference  snapshot-util 0.1 main/LIMIT.
#                      ^^^ reference  snapshot-util 0.1 main/run().(box)
#                                 ^^^^^ reference local 0
    return LIMIT
#          ^^^^^ reference  snapshot-util 0.1 main/LIMIT.
"#,
    );
}

/// A member reference through a receiver whose class overrides the member names the override,
/// not the base's declaration: a method, a class attribute, an instance attribute and a
/// Django-style `objects` manager (GOAL.md B1). Inside the base class, `self` is the base, so
/// its references name the base's declarations.
#[test]
fn test_reference_to_overriding_member() {
    assert_index(
        &[(
            "main",
            r#"
class Manager: ...
class PartManager(Manager): ...

class Model:
    objects = Manager()
    limit = 1

    def __init__(self) -> None:
        self.label = "model"

    def run(self) -> None:
        self.run()

class Part(Model):
    objects = PartManager()
    limit = 2

    def __init__(self) -> None:
        self.label = "part"

    def run(self) -> None: ...

def use(part: Part) -> None:
    part.run()
    part.limit
    part.label
    Part.objects
    Part.limit
"#,
        )],
        r#"
# main.py
# < definition scip-python python snapshot-util 0.1 main/__init__:

class Manager: ...
#     ^^^^^^^ definition  snapshot-util 0.1 main/Manager#
#      enclosing [1, 0, 18]
class PartManager(Manager): ...
#     ^^^^^^^^^^^ definition  snapshot-util 0.1 main/PartManager#
#      enclosing [2, 0, 31]
#      relationship implementation  snapshot-util 0.1 main/Manager#
#                 ^^^^^^^ reference  snapshot-util 0.1 main/Manager#

class Model:
#     ^^^^^ definition  snapshot-util 0.1 main/Model#
#      enclosing [4, 0, 12, 18]
    objects = Manager()
#   ^^^^^^^ definition  snapshot-util 0.1 main/Model#objects.
#    enclosing [5, 4, 23]
#             ^^^^^^^ reference  snapshot-util 0.1 main/Manager#
    limit = 1
#   ^^^^^ definition  snapshot-util 0.1 main/Model#limit.
#    enclosing [6, 4, 13]

    def __init__(self) -> None:
#       ^^^^^^^^ definition  snapshot-util 0.1 main/Model#__init__().
#        enclosing [8, 4, 9, 28]
#                ^^^^ definition  snapshot-util 0.1 main/Model#__init__().(self)
        self.label = "model"
#       ^^^^ reference  snapshot-util 0.1 main/Model#__init__().(self)
#            ^^^^^ definition  snapshot-util 0.1 main/Model#label.
#             enclosing [9, 8, 28]

    def run(self) -> None:
#       ^^^ definition  snapshot-util 0.1 main/Model#run().
#        enclosing [11, 4, 12, 18]
#           ^^^^ definition  snapshot-util 0.1 main/Model#run().(self)
        self.run()
#       ^^^^ reference  snapshot-util 0.1 main/Model#run().(self)
#            ^^^ reference  snapshot-util 0.1 main/Model#run().

class Part(Model):
#     ^^^^ definition  snapshot-util 0.1 main/Part#
#      enclosing [14, 0, 21, 30]
#      relationship implementation  snapshot-util 0.1 main/Model#
#          ^^^^^ reference  snapshot-util 0.1 main/Model#
    objects = PartManager()
#   ^^^^^^^ definition  snapshot-util 0.1 main/Part#objects.
#    enclosing [15, 4, 27]
#             ^^^^^^^^^^^ reference  snapshot-util 0.1 main/PartManager#
    limit = 2
#   ^^^^^ definition  snapshot-util 0.1 main/Part#limit.
#    enclosing [16, 4, 13]

    def __init__(self) -> None:
#       ^^^^^^^^ definition  snapshot-util 0.1 main/Part#__init__().
#        enclosing [18, 4, 19, 27]
#                ^^^^ definition  snapshot-util 0.1 main/Part#__init__().(self)
        self.label = "part"
#       ^^^^ reference  snapshot-util 0.1 main/Part#__init__().(self)
#            ^^^^^ definition  snapshot-util 0.1 main/Part#label.
#             enclosing [19, 8, 27]

    def run(self) -> None: ...
#       ^^^ definition  snapshot-util 0.1 main/Part#run().
#        enclosing [21, 4, 30]
#        relationship implementation  snapshot-util 0.1 main/Model#run().
#           ^^^^ definition  snapshot-util 0.1 main/Part#run().(self)

def use(part: Part) -> None:
#   ^^^ definition  snapshot-util 0.1 main/use().
#    enclosing [23, 0, 28, 14]
#       ^^^^ definition  snapshot-util 0.1 main/use().(part)
#             ^^^^ reference  snapshot-util 0.1 main/Part#
    part.run()
#   ^^^^ reference  snapshot-util 0.1 main/use().(part)
#        ^^^ reference  snapshot-util 0.1 main/Part#run().
    part.limit
#   ^^^^ reference  snapshot-util 0.1 main/use().(part)
#        ^^^^^ reference  snapshot-util 0.1 main/Part#limit.
    part.label
#   ^^^^ reference  snapshot-util 0.1 main/use().(part)
#        ^^^^^ reference  snapshot-util 0.1 main/Part#label.
    Part.objects
#   ^^^^ reference  snapshot-util 0.1 main/Part#
#        ^^^^^^^ reference  snapshot-util 0.1 main/Part#objects.
    Part.limit
#   ^^^^ reference  snapshot-util 0.1 main/Part#
#        ^^^^^ reference  snapshot-util 0.1 main/Part#limit.
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

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/aliased_import/.
// Matches scip-python's output, except that scip-python gives the alias `A` of
// `import aliased as A` a local symbol, and pyrefly the module `aliased/__init__:` it names.
#[test]
fn test_ported_aliased_import() {
    assert_index(
        &[
            (
                "actual",
                r#"import aliased
import aliased as A

print(A.SOME_CONSTANT)
"#,
            ),
            (
                "aliased",
                r#"SOME_CONSTANT = "Some Constant"
"#,
            ),
        ],
        r#"
# actual.py
# < definition scip-python python snapshot-util 0.1 actual/__init__:
import aliased
#      ^^^^^^^ reference[import]  snapshot-util 0.1 aliased/__init__:
import aliased as A
#      ^^^^^^^ reference[import]  snapshot-util 0.1 aliased/__init__:
#                 ^ reference[import]  snapshot-util 0.1 aliased/__init__:

print(A.SOME_CONSTANT)
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^ reference  snapshot-util 0.1 aliased/__init__:
#       ^^^^^^^^^^^^^ reference  snapshot-util 0.1 aliased/SOME_CONSTANT.

# aliased.py
# < definition scip-python python snapshot-util 0.1 aliased/__init__:
SOME_CONSTANT = "Some Constant"
#^^^^^^^^^^^^ definition  snapshot-util 0.1 aliased/SOME_CONSTANT.
# enclosing [0, 0, 31]
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/builtin_imports/.
// Matches scip-python's output.
#[test]
fn test_ported_builtin_imports() {
    assert_index(
        &[(
            "builtin_imports",
            r#"# format-options: showDocs

import re
from typing import Callable, Optional

print(re, Callable, Optional)
"#,
        )],
        r#"
# builtin_imports.py
# < definition scip-python python snapshot-util 0.1 builtin_imports/__init__:
# format-options: showDocs

import re
#      ^^ reference[import]  python-stdlib 3.12 re/__init__:
from typing import Callable, Optional
#    ^^^^^^ reference[import]  python-stdlib 3.12 typing/__init__:
#                  ^^^^^^^^ reference[import]  python-stdlib 3.12 typing/Callable.
#                            ^^^^^^^^ reference[import]  python-stdlib 3.12 typing/Optional.

print(re, Callable, Optional)
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^^ reference  python-stdlib 3.12 re/__init__:
#         ^^^^^^^^ reference  python-stdlib 3.12 typing/Callable.
#                   ^^^^^^^^ reference  python-stdlib 3.12 typing/Optional.
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/class_nohint/.
// Matches scip-python's output.
#[test]
fn test_ported_class_nohint() {
    assert_index(
        &[(
            "class_nohint",
            r#"# format-options: showDocs

class Example:
    # Note, only y has a type hint
    y: int

    def __init__(self, in_val):
        self.x = in_val
        self.x = self.x + 1
        self.y = in_val

    def something(self):
        print(self.x)
        print(self.y)
"#,
        )],
        r#"
# class_nohint.py
# < definition scip-python python snapshot-util 0.1 class_nohint/__init__:
# format-options: showDocs

class Example:
#     ^^^^^^^ definition  snapshot-util 0.1 class_nohint/Example#
#      enclosing [2, 0, 13, 21]
    # Note, only y has a type hint
    y: int
#   ^ definition  snapshot-util 0.1 class_nohint/Example#y.
#    enclosing [4, 4, 10]
#      ^^^ reference  python-stdlib 3.12 builtins/int#

    def __init__(self, in_val):
#       ^^^^^^^^ definition  snapshot-util 0.1 class_nohint/Example#__init__().
#        enclosing [6, 4, 9, 23]
#                ^^^^ definition  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#                      ^^^^^^ definition  snapshot-util 0.1 class_nohint/Example#__init__().(in_val)
        self.x = in_val
#       ^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#            ^ definition  snapshot-util 0.1 class_nohint/Example#x.
#             enclosing [7, 8, 23]
#                ^^^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(in_val)
        self.x = self.x + 1
#       ^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#            ^ reference[write]  snapshot-util 0.1 class_nohint/Example#x.
#                ^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#                     ^ reference  snapshot-util 0.1 class_nohint/Example#x.
        self.y = in_val
#       ^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#            ^ reference[write]  snapshot-util 0.1 class_nohint/Example#y.
#                ^^^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(in_val)

    def something(self):
#       ^^^^^^^^^ definition  snapshot-util 0.1 class_nohint/Example#something().
#        enclosing [11, 4, 13, 21]
#                 ^^^^ definition  snapshot-util 0.1 class_nohint/Example#something().(self)
        print(self.x)
#       ^^^^^ reference  python-stdlib 3.12 builtins/print().
#             ^^^^ reference  snapshot-util 0.1 class_nohint/Example#something().(self)
#                  ^ reference  snapshot-util 0.1 class_nohint/Example#x.
        print(self.y)
#       ^^^^^ reference  python-stdlib 3.12 builtins/print().
#             ^^^^ reference  snapshot-util 0.1 class_nohint/Example#something().(self)
#                  ^ reference  snapshot-util 0.1 class_nohint/Example#y.
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/comprehensions/.
// Matches scip-python's output.
#[test]
fn test_ported_comprehensions() {
    assert_index(
        &[(
            "comp",
            r#"def something(x):
    return 2 * x

_ = [x for x in "should be local 0"]
_ = [something(x) for x in "should be local 1"]
_ = [something(x) for x in "should be local 2" if x == "s"]
_ = {k: x for (k, x) in enumerate(range(10))}


asdf = (var for var in [1, 2, 3] if var % 2 == 0)
for var in asdf:
    print(var)

# TODO: ListComprehensions with if
# TODO: Dictionary comprehensions

"#,
        )],
        r#"
# comp.py
# < definition scip-python python snapshot-util 0.1 comp/__init__:
def something(x):
#   ^^^^^^^^^ definition  snapshot-util 0.1 comp/something().
#    enclosing [0, 0, 1, 16]
#             ^ definition  snapshot-util 0.1 comp/something().(x)
    return 2 * x
#              ^ reference  snapshot-util 0.1 comp/something().(x)

_ = [x for x in "should be local 0"]
#    ^ reference local 0
#          ^ definition local 0
_ = [something(x) for x in "should be local 1"]
#    ^^^^^^^^^ reference  snapshot-util 0.1 comp/something().
#              ^ reference local 1
#                     ^ definition local 1
_ = [something(x) for x in "should be local 2" if x == "s"]
#    ^^^^^^^^^ reference  snapshot-util 0.1 comp/something().
#              ^ reference local 2
#                     ^ definition local 2
#                                                 ^ reference local 2
_ = {k: x for (k, x) in enumerate(range(10))}
#    ^ reference local 3
#       ^ reference local 4
#              ^ definition local 3
#                 ^ definition local 4
#                       ^^^^^^^^^ reference  python-stdlib 3.12 builtins/enumerate#
#                                 ^^^^^ reference  python-stdlib 3.12 builtins/range#


asdf = (var for var in [1, 2, 3] if var % 2 == 0)
#^^^ definition  snapshot-util 0.1 comp/asdf.
# enclosing [9, 0, 49]
#       ^^^ reference local 5
#               ^^^ definition local 5
#                                   ^^^ reference local 5
for var in asdf:
#   ^^^ definition  snapshot-util 0.1 comp/var.
#    enclosing [10, 0, 11, 14]
#          ^^^^ reference  snapshot-util 0.1 comp/asdf.
    print(var)
#   ^^^^^ reference  python-stdlib 3.12 builtins/print().
#         ^^^ reference  snapshot-util 0.1 comp/var.

# TODO: ListComprehensions with if
# TODO: Dictionary comprehensions
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/dunder_vars/.
// Matches scip-python's output, except `__name__`, which scip-python names with the invented
// `builtins/__name__#` and pyrefly leaves without an occurrence: an implicit module attribute
// has no definition site (`module-namespace` and `builtin-named-attribute` in
// scip-goal/ADJUDICATIONS.md).
#[test]
fn test_ported_dunder_vars() {
    assert_index(
        &[(
            "__main__",
            r#"
if __name__ == '__main__':
    print("main")

"#,
        )],
        r#"
# __main__.py
# < definition scip-python python snapshot-util 0.1 __main__/__init__:

if __name__ == '__main__':
    print("main")
#   ^^^^^ reference  python-stdlib 3.12 builtins/print().
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/enclosing_range_class/.
// The occurrences and enclosing ranges match scip-python's, which its snapshots draw as start
// and end markers.
#[test]
fn test_ported_enclosing_range_class() {
    assert_index(
        &[
            (
                "advanced",
                r#"# format-options: showRanges
def class_decorator(cls):
    def wrapper(*args, **kwargs):
        return cls(*args, **kwargs)
    return wrapper

@class_decorator
class Test:
    def __init__(self, x: float):
        self.x = x

    def test(self) -> float:
        return self.x
"#,
            ),
            (
                "simple",
                r#"# format-options: showRanges
class Test():
  pass
"#,
            ),
        ],
        r#"
# advanced.py
# < definition scip-python python snapshot-util 0.1 advanced/__init__:
# format-options: showRanges
def class_decorator(cls):
#   ^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 advanced/class_decorator().
#    enclosing [1, 0, 4, 18]
#                   ^^^ definition  snapshot-util 0.1 advanced/class_decorator().(cls)
    def wrapper(*args, **kwargs):
#       ^^^^^^^ definition  snapshot-util 0.1 advanced/class_decorator().wrapper().
#        enclosing [2, 4, 3, 35]
#                ^^^^ definition  snapshot-util 0.1 advanced/class_decorator().wrapper().(args)
#                        ^^^^^^ definition  snapshot-util 0.1 advanced/class_decorator().wrapper().(kwargs)
        return cls(*args, **kwargs)
#              ^^^ reference  snapshot-util 0.1 advanced/class_decorator().(cls)
#                   ^^^^ reference  snapshot-util 0.1 advanced/class_decorator().wrapper().(args)
#                           ^^^^^^ reference  snapshot-util 0.1 advanced/class_decorator().wrapper().(kwargs)
    return wrapper
#          ^^^^^^^ reference  snapshot-util 0.1 advanced/class_decorator().wrapper().

@class_decorator
#^^^^^^^^^^^^^^^ reference  snapshot-util 0.1 advanced/class_decorator().
class Test:
#     ^^^^ definition  snapshot-util 0.1 advanced/Test#
#      enclosing [7, 0, 12, 21]
    def __init__(self, x: float):
#       ^^^^^^^^ definition  snapshot-util 0.1 advanced/Test#__init__().
#        enclosing [8, 4, 9, 18]
#                ^^^^ definition  snapshot-util 0.1 advanced/Test#__init__().(self)
#                      ^ definition  snapshot-util 0.1 advanced/Test#__init__().(x)
#                         ^^^^^ reference  python-stdlib 3.12 builtins/float#
        self.x = x
#       ^^^^ reference  snapshot-util 0.1 advanced/Test#__init__().(self)
#            ^ definition  snapshot-util 0.1 advanced/Test#x.
#             enclosing [9, 8, 18]
#                ^ reference  snapshot-util 0.1 advanced/Test#__init__().(x)

    def test(self) -> float:
#       ^^^^ definition  snapshot-util 0.1 advanced/Test#test().
#        enclosing [11, 4, 12, 21]
#            ^^^^ definition  snapshot-util 0.1 advanced/Test#test().(self)
#                     ^^^^^ reference  python-stdlib 3.12 builtins/float#
        return self.x
#              ^^^^ reference  snapshot-util 0.1 advanced/Test#test().(self)
#                   ^ reference  snapshot-util 0.1 advanced/Test#x.

# simple.py
# < definition scip-python python snapshot-util 0.1 simple/__init__:
# format-options: showRanges
class Test():
#     ^^^^ definition  snapshot-util 0.1 simple/Test#
#      enclosing [1, 0, 2, 6]
  pass
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/enclosing_range_function/.
// The occurrences and enclosing ranges match scip-python's, which its snapshots draw as start
// and end markers.
#[test]
fn test_ported_enclosing_range_function() {
    assert_index(
        &[
            (
                "advanced",
                r#"# format-options: showRanges
def decorator(func):
    def wrapper(*args, **kwargs):
        return func(*args, **kwargs)
    return wrapper

@decorator
def func(x: float) -> float:
    test = x

    return test
"#,
            ),
            (
                "simple",
                r#"# format-options: showRanges
def simple(x):
  return x
"#,
            ),
        ],
        r#"
# advanced.py
# < definition scip-python python snapshot-util 0.1 advanced/__init__:
# format-options: showRanges
def decorator(func):
#   ^^^^^^^^^ definition  snapshot-util 0.1 advanced/decorator().
#    enclosing [1, 0, 4, 18]
#             ^^^^ definition  snapshot-util 0.1 advanced/decorator().(func)
    def wrapper(*args, **kwargs):
#       ^^^^^^^ definition  snapshot-util 0.1 advanced/decorator().wrapper().
#        enclosing [2, 4, 3, 36]
#                ^^^^ definition  snapshot-util 0.1 advanced/decorator().wrapper().(args)
#                        ^^^^^^ definition  snapshot-util 0.1 advanced/decorator().wrapper().(kwargs)
        return func(*args, **kwargs)
#              ^^^^ reference  snapshot-util 0.1 advanced/decorator().(func)
#                    ^^^^ reference  snapshot-util 0.1 advanced/decorator().wrapper().(args)
#                            ^^^^^^ reference  snapshot-util 0.1 advanced/decorator().wrapper().(kwargs)
    return wrapper
#          ^^^^^^^ reference  snapshot-util 0.1 advanced/decorator().wrapper().

@decorator
#^^^^^^^^^ reference  snapshot-util 0.1 advanced/decorator().
def func(x: float) -> float:
#   ^^^^ definition  snapshot-util 0.1 advanced/func().
#    enclosing [7, 0, 10, 15]
#        ^ definition  snapshot-util 0.1 advanced/func().(x)
#           ^^^^^ reference  python-stdlib 3.12 builtins/float#
#                     ^^^^^ reference  python-stdlib 3.12 builtins/float#
    test = x
#   ^^^^ definition local 0
#          ^ reference  snapshot-util 0.1 advanced/func().(x)

    return test
#          ^^^^ reference local 0

# simple.py
# < definition scip-python python snapshot-util 0.1 simple/__init__:
# format-options: showRanges
def simple(x):
#   ^^^^^^ definition  snapshot-util 0.1 simple/simple().
#    enclosing [1, 0, 2, 10]
#          ^ definition  snapshot-util 0.1 simple/simple().(x)
  return x
#        ^ reference  snapshot-util 0.1 simple/simple().(x)
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/f_string/.
// Matches scip-python's output.
#[test]
fn test_ported_f_string() {
    assert_index(
        &[(
            "fstring",
            r#"var = ", world!"

print(f"var: hello {var}")
"#,
        )],
        r#"
# fstring.py
# < definition scip-python python snapshot-util 0.1 fstring/__init__:
var = ", world!"
#^^ definition  snapshot-util 0.1 fstring/var.
# enclosing [0, 0, 16]

print(f"var: hello {var}")
#^^^^ reference  python-stdlib 3.12 builtins/print().
#                   ^^^ reference  snapshot-util 0.1 fstring/var.
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/file_from_module_import/.
// Matches scip-python's output. scip-python's renderer draws the one-character definition
// `X` at the start of a line without a caret.
#[test]
fn test_ported_file_from_module_import() {
    assert_index(
        &[
            (
                "abc.file",
                r#"from xyz import nested_file

print(nested_file.X)
"#,
            ),
            (
                "file_from_module",
                r#"from xyz import nested_file

print(nested_file.X)
"#,
            ),
            ("xyz", r#""#),
            (
                "xyz.nested_file",
                r#"X = "a constant"
"#,
            ),
        ],
        r#"
# abc/file.py
# < definition scip-python python snapshot-util 0.1 `abc.file`/__init__:
from xyz import nested_file
#    ^^^ reference[import]  snapshot-util 0.1 xyz/__init__:
#               ^^^^^^^^^^^ reference[import]  snapshot-util 0.1 `xyz.nested_file`/__init__:

print(nested_file.X)
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^^^^^^^^^^^ reference  snapshot-util 0.1 `xyz.nested_file`/__init__:
#                 ^ reference  snapshot-util 0.1 `xyz.nested_file`/X.

# file_from_module.py
# < definition scip-python python snapshot-util 0.1 file_from_module/__init__:
from xyz import nested_file
#    ^^^ reference[import]  snapshot-util 0.1 xyz/__init__:
#               ^^^^^^^^^^^ reference[import]  snapshot-util 0.1 `xyz.nested_file`/__init__:

print(nested_file.X)
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^^^^^^^^^^^ reference  snapshot-util 0.1 `xyz.nested_file`/__init__:
#                 ^ reference  snapshot-util 0.1 `xyz.nested_file`/X.

# xyz.py
# < definition scip-python python snapshot-util 0.1 xyz/__init__:

# xyz/nested_file.py
# < definition scip-python python snapshot-util 0.1 `xyz.nested_file`/__init__:
X = "a constant"
#^ definition  snapshot-util 0.1 `xyz.nested_file`/X.
# enclosing [0, 0, 16]
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/graph_1278/.
// Matches scip-python's output, except the relationship to `typing.Generic`, which the report
// leaves out (`relationship-special-form` in scip-goal/ADJUDICATIONS.md).
#[test]
fn test_ported_graph_1278() {
    assert_index(
        &[(
            "mwe",
            r#"from typing import TypeVar, Generic, Callable, Iterator, ParamSpec

_T_co = TypeVar("_T_co")
_P = ParamSpec("_P")

class X(Generic[_T_co]):
    pass

def decorate(func: Callable[_P, Iterator[_T_co]]) -> Callable[_P, X[_T_co]]: ...

class Foo:
    @decorate
    def foo(self) -> Iterator[None]: ...

@decorate
def noop():
    yield

class FooImpl(Foo):
    def foo(self):
        return noop()"#,
        )],
        r#"
# mwe.py
# < definition scip-python python snapshot-util 0.1 mwe/__init__:
from typing import TypeVar, Generic, Callable, Iterator, ParamSpec
#    ^^^^^^ reference[import]  python-stdlib 3.12 typing/__init__:
#                  ^^^^^^^ reference[import]  python-stdlib 3.12 typing/TypeVar#
#                           ^^^^^^^ reference[import]  python-stdlib 3.12 typing/Generic.
#                                    ^^^^^^^^ reference[import]  python-stdlib 3.12 typing/Callable.
#                                              ^^^^^^^^ reference[import]  python-stdlib 3.12 typing/Iterator#
#                                                        ^^^^^^^^^ reference[import]  python-stdlib 3.12 typing/ParamSpec#

_T_co = TypeVar("_T_co")
#^^^^ definition  snapshot-util 0.1 mwe/_T_co.
# enclosing [2, 0, 24]
#       ^^^^^^^ reference  python-stdlib 3.12 typing/TypeVar#
_P = ParamSpec("_P")
#^ definition  snapshot-util 0.1 mwe/_P.
# enclosing [3, 0, 20]
#    ^^^^^^^^^ reference  python-stdlib 3.12 typing/ParamSpec#

class X(Generic[_T_co]):
#     ^ definition  snapshot-util 0.1 mwe/X#
#      enclosing [5, 0, 6, 8]
#       ^^^^^^^ reference  python-stdlib 3.12 typing/Generic.
#               ^^^^^ reference  snapshot-util 0.1 mwe/_T_co.
    pass

def decorate(func: Callable[_P, Iterator[_T_co]]) -> Callable[_P, X[_T_co]]: ...
#   ^^^^^^^^ definition  snapshot-util 0.1 mwe/decorate().
#    enclosing [8, 0, 80]
#            ^^^^ definition  snapshot-util 0.1 mwe/decorate().(func)
#                  ^^^^^^^^ reference  python-stdlib 3.12 typing/Callable.
#                           ^^ reference  snapshot-util 0.1 mwe/_P.
#                               ^^^^^^^^ reference  python-stdlib 3.12 typing/Iterator#
#                                        ^^^^^ reference  snapshot-util 0.1 mwe/_T_co.
#                                                    ^^^^^^^^ reference  python-stdlib 3.12 typing/Callable.
#                                                             ^^ reference  snapshot-util 0.1 mwe/_P.
#                                                                 ^ reference  snapshot-util 0.1 mwe/X#
#                                                                   ^^^^^ reference  snapshot-util 0.1 mwe/_T_co.

class Foo:
#     ^^^ definition  snapshot-util 0.1 mwe/Foo#
#      enclosing [10, 0, 12, 40]
    @decorate
#    ^^^^^^^^ reference  snapshot-util 0.1 mwe/decorate().
    def foo(self) -> Iterator[None]: ...
#       ^^^ definition  snapshot-util 0.1 mwe/Foo#foo().
#        enclosing [12, 4, 40]
#           ^^^^ definition  snapshot-util 0.1 mwe/Foo#foo().(self)
#                    ^^^^^^^^ reference  python-stdlib 3.12 typing/Iterator#

@decorate
#^^^^^^^^ reference  snapshot-util 0.1 mwe/decorate().
def noop():
#   ^^^^ definition  snapshot-util 0.1 mwe/noop().
#    enclosing [15, 0, 16, 9]
    yield

class FooImpl(Foo):
#     ^^^^^^^ definition  snapshot-util 0.1 mwe/FooImpl#
#      enclosing [18, 0, 20, 21]
#      relationship implementation  snapshot-util 0.1 mwe/Foo#
#             ^^^ reference  snapshot-util 0.1 mwe/Foo#
    def foo(self):
#       ^^^ definition  snapshot-util 0.1 mwe/FooImpl#foo().
#        enclosing [19, 4, 20, 21]
#        relationship implementation  snapshot-util 0.1 mwe/Foo#foo().
#           ^^^^ definition  snapshot-util 0.1 mwe/FooImpl#foo().(self)
        return noop()
#              ^^^^ reference  snapshot-util 0.1 mwe/noop().
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/nameparts_import/.
// Differs from scip-python's output: scip-python gives `importlib.resources` in an expression
// one occurrence across the dotted name (`dotted-module-span`) and `read_text` a local symbol;
// pyrefly emits one occurrence per segment and resolves `read_text`.
#[test]
fn test_ported_nameparts_import() {
    assert_index(
        &[(
            "nameparts",
            r#"import importlib.resources

importlib.resources.read_text('pre_commit.resources', 'filename')
importlib.resources.read_text('pre_commit.resources', 'filename')
"#,
        )],
        r#"
# nameparts.py
# < definition scip-python python snapshot-util 0.1 nameparts/__init__:
import importlib.resources
#      ^^^^^^^^^^^^^^^^^^^ reference[import]  python-stdlib 3.12 `importlib.resources`/__init__:

importlib.resources.read_text('pre_commit.resources', 'filename')
#^^^^^^^^ reference  python-stdlib 3.12 importlib/__init__:
#         ^^^^^^^^^ reference  python-stdlib 3.12 `importlib.resources`/__init__:
#                   ^^^^^^^^^ reference  python-stdlib 3.12 `importlib.resources._functional`/read_text().
importlib.resources.read_text('pre_commit.resources', 'filename')
#^^^^^^^^ reference  python-stdlib 3.12 importlib/__init__:
#         ^^^^^^^^^ reference  python-stdlib 3.12 `importlib.resources`/__init__:
#                   ^^^^^^^^^ reference  python-stdlib 3.12 `importlib.resources._functional`/read_text().
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/nested_items/.
// scip-python's project puts `src/` on the search path, so the modules here are named
// relative to it. scip-python names each definition by its path from the project root
// (`src.foo.bar`) but the module references in imports by their import name (`foo.bar`);
// pyrefly names both `foo.bar`. In `long_importer`, scip-python spans `foo.bar.baz.mod` with
// one occurrence (`dotted-module-span`).
#[test]
fn test_ported_nested_items() {
    assert_index(
        &[
            ("foo", r#""#),
            (
                "foo.bar",
                r#"class InitClass:
    init_item = 10
"#,
            ),
            (
                "foo.bar.baz.mod",
                r#"class SuchNestedMuchWow:
    class_item: int = 42

class AnotherNestedMuchWow:
    other_item: int = 42
"#,
            ),
            (
                "importer",
                r#"from foo.bar import InitClass
from foo.bar.baz.mod import SuchNestedMuchWow, AnotherNestedMuchWow

print(SuchNestedMuchWow().class_item)
print(AnotherNestedMuchWow().other_item)
print(InitClass().init_item)
"#,
            ),
            (
                "long_importer",
                r#"import foo.bar.baz.mod

print(foo.bar.baz.mod.SuchNestedMuchWow)
"#,
            ),
        ],
        r#"
# foo.py
# < definition scip-python python snapshot-util 0.1 foo/__init__:

# foo/bar.py
# < definition scip-python python snapshot-util 0.1 `foo.bar`/__init__:
class InitClass:
#     ^^^^^^^^^ definition  snapshot-util 0.1 `foo.bar`/InitClass#
#      enclosing [0, 0, 1, 18]
    init_item = 10
#   ^^^^^^^^^ definition  snapshot-util 0.1 `foo.bar`/InitClass#init_item.
#    enclosing [1, 4, 18]

# foo/bar/baz/mod.py
# < definition scip-python python snapshot-util 0.1 `foo.bar.baz.mod`/__init__:
class SuchNestedMuchWow:
#     ^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 `foo.bar.baz.mod`/SuchNestedMuchWow#
#      enclosing [0, 0, 1, 24]
    class_item: int = 42
#   ^^^^^^^^^^ definition  snapshot-util 0.1 `foo.bar.baz.mod`/SuchNestedMuchWow#class_item.
#    enclosing [1, 4, 24]
#               ^^^ reference  python-stdlib 3.12 builtins/int#

class AnotherNestedMuchWow:
#     ^^^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 `foo.bar.baz.mod`/AnotherNestedMuchWow#
#      enclosing [3, 0, 4, 24]
    other_item: int = 42
#   ^^^^^^^^^^ definition  snapshot-util 0.1 `foo.bar.baz.mod`/AnotherNestedMuchWow#other_item.
#    enclosing [4, 4, 24]
#               ^^^ reference  python-stdlib 3.12 builtins/int#

# importer.py
# < definition scip-python python snapshot-util 0.1 importer/__init__:
from foo.bar import InitClass
#    ^^^^^^^ reference[import]  snapshot-util 0.1 `foo.bar`/__init__:
#                   ^^^^^^^^^ reference[import]  snapshot-util 0.1 `foo.bar`/InitClass#
from foo.bar.baz.mod import SuchNestedMuchWow, AnotherNestedMuchWow
#    ^^^^^^^^^^^^^^^ reference[import]  snapshot-util 0.1 `foo.bar.baz.mod`/__init__:
#                           ^^^^^^^^^^^^^^^^^ reference[import]  snapshot-util 0.1 `foo.bar.baz.mod`/SuchNestedMuchWow#
#                                              ^^^^^^^^^^^^^^^^^^^^ reference[import]  snapshot-util 0.1 `foo.bar.baz.mod`/AnotherNestedMuchWow#

print(SuchNestedMuchWow().class_item)
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^^^^^^^^^^^^^^^^^ reference  snapshot-util 0.1 `foo.bar.baz.mod`/SuchNestedMuchWow#
#                         ^^^^^^^^^^ reference  snapshot-util 0.1 `foo.bar.baz.mod`/SuchNestedMuchWow#class_item.
print(AnotherNestedMuchWow().other_item)
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^^^^^^^^^^^^^^^^^^^^ reference  snapshot-util 0.1 `foo.bar.baz.mod`/AnotherNestedMuchWow#
#                            ^^^^^^^^^^ reference  snapshot-util 0.1 `foo.bar.baz.mod`/AnotherNestedMuchWow#other_item.
print(InitClass().init_item)
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^^^^^^^^^ reference  snapshot-util 0.1 `foo.bar`/InitClass#
#                 ^^^^^^^^^ reference  snapshot-util 0.1 `foo.bar`/InitClass#init_item.

# long_importer.py
# < definition scip-python python snapshot-util 0.1 long_importer/__init__:
import foo.bar.baz.mod
#      ^^^^^^^^^^^^^^^ reference[import]  snapshot-util 0.1 `foo.bar.baz.mod`/__init__:

print(foo.bar.baz.mod.SuchNestedMuchWow)
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^^^ reference  snapshot-util 0.1 foo/__init__:
#         ^^^ reference  snapshot-util 0.1 `foo.bar`/__init__:
#                 ^^^ reference  snapshot-util 0.1 `foo.bar.baz.mod`/__init__:
#                     ^^^^^^^^^^^^^^^^^ reference  snapshot-util 0.1 `foo.bar.baz.mod`/SuchNestedMuchWow#
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/relative_import/.
// Matches scip-python's output.
#[test]
fn test_ported_relative_import() {
    assert_index(
        &[
            (
                "bar",
                r#"from .foo import exported_function
from .foo import this_class

if True:
    exported_function()

    this_class.exported_function()
"#,
            ),
            (
                "foo",
                r#"def exported_function():
    return "function"

class MyClass:
    """This is a class and it is cool"""

    def __init__(self):
        pass

    def exported_function(self):
        return "exported"

this_class = MyClass()
"#,
            ),
        ],
        r#"
# bar.py
# < definition scip-python python snapshot-util 0.1 bar/__init__:
from .foo import exported_function
#    ^^^^ reference[import]  snapshot-util 0.1 foo/__init__:
#                ^^^^^^^^^^^^^^^^^ reference[import]  snapshot-util 0.1 foo/exported_function().
from .foo import this_class
#    ^^^^ reference[import]  snapshot-util 0.1 foo/__init__:
#                ^^^^^^^^^^ reference[import]  snapshot-util 0.1 foo/this_class.

if True:
    exported_function()
#   ^^^^^^^^^^^^^^^^^ reference  snapshot-util 0.1 foo/exported_function().

    this_class.exported_function()
#   ^^^^^^^^^^ reference  snapshot-util 0.1 foo/this_class.
#              ^^^^^^^^^^^^^^^^^ reference  snapshot-util 0.1 foo/MyClass#exported_function().

# foo.py
# < definition scip-python python snapshot-util 0.1 foo/__init__:
def exported_function():
#   ^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 foo/exported_function().
#    enclosing [0, 0, 1, 21]
    return "function"

class MyClass:
#     ^^^^^^^ definition  snapshot-util 0.1 foo/MyClass#
#      enclosing [3, 0, 10, 25]
    """This is a class and it is cool"""

    def __init__(self):
#       ^^^^^^^^ definition  snapshot-util 0.1 foo/MyClass#__init__().
#        enclosing [6, 4, 7, 12]
#                ^^^^ definition  snapshot-util 0.1 foo/MyClass#__init__().(self)
        pass

    def exported_function(self):
#       ^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 foo/MyClass#exported_function().
#        enclosing [9, 4, 10, 25]
#                         ^^^^ definition  snapshot-util 0.1 foo/MyClass#exported_function().(self)
        return "exported"

this_class = MyClass()
#^^^^^^^^^ definition  snapshot-util 0.1 foo/this_class.
# enclosing [12, 0, 22]
#            ^^^^^^^ reference  snapshot-util 0.1 foo/MyClass#
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/request_goofiness/.
// Matches scip-python's output, except the version of the external package `requests`:
// scip-python reads `2.0.0` from its environment file, and pyrefly, which resolves `requests`
// to its bundled stubs without an installed version, writes the placeholder `.`.
#[test]
fn test_ported_request_goofiness() {
    assert_index(
        &[(
            "goofy",
            r#"import requests

print(requests.get("https://sourcegraph.com"))
"#,
        )],
        r#"
# goofy.py
# < definition scip-python python snapshot-util 0.1 goofy/__init__:
import requests
#      ^^^^^^^^ reference[import]  requests . requests/__init__:

print(requests.get("https://sourcegraph.com"))
#^^^^ reference  python-stdlib 3.12 builtins/print().
#     ^^^^^^^^ reference  requests . requests/__init__:
#              ^^^ reference  requests . `requests.api`/get().
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/single_class/.
// Matches scip-python's output.
#[test]
fn test_ported_single_class() {
    assert_index(
        &[(
            "src.single_class",
            r#"class ExampleClass:
    a: int
    b: int
    c: str

    static_var = "Hello World"

    def __init__(self, a: int, b: int):
        local_c = ", world!"

        self.a = a
        self.b = b
        self.c = "hello" + local_c
"#,
        )],
        r#"
# src/single_class.py
# < definition scip-python python snapshot-util 0.1 `src.single_class`/__init__:
class ExampleClass:
#     ^^^^^^^^^^^^ definition  snapshot-util 0.1 `src.single_class`/ExampleClass#
#      enclosing [0, 0, 12, 34]
    a: int
#   ^ definition  snapshot-util 0.1 `src.single_class`/ExampleClass#a.
#    enclosing [1, 4, 10]
#      ^^^ reference  python-stdlib 3.12 builtins/int#
    b: int
#   ^ definition  snapshot-util 0.1 `src.single_class`/ExampleClass#b.
#    enclosing [2, 4, 10]
#      ^^^ reference  python-stdlib 3.12 builtins/int#
    c: str
#   ^ definition  snapshot-util 0.1 `src.single_class`/ExampleClass#c.
#    enclosing [3, 4, 10]
#      ^^^ reference  python-stdlib 3.12 builtins/str#

    static_var = "Hello World"
#   ^^^^^^^^^^ definition  snapshot-util 0.1 `src.single_class`/ExampleClass#static_var.
#    enclosing [5, 4, 30]

    def __init__(self, a: int, b: int):
#       ^^^^^^^^ definition  snapshot-util 0.1 `src.single_class`/ExampleClass#__init__().
#        enclosing [7, 4, 12, 34]
#                ^^^^ definition  snapshot-util 0.1 `src.single_class`/ExampleClass#__init__().(self)
#                      ^ definition  snapshot-util 0.1 `src.single_class`/ExampleClass#__init__().(a)
#                         ^^^ reference  python-stdlib 3.12 builtins/int#
#                              ^ definition  snapshot-util 0.1 `src.single_class`/ExampleClass#__init__().(b)
#                                 ^^^ reference  python-stdlib 3.12 builtins/int#
        local_c = ", world!"
#       ^^^^^^^ definition local 0

        self.a = a
#       ^^^^ reference  snapshot-util 0.1 `src.single_class`/ExampleClass#__init__().(self)
#            ^ reference[write]  snapshot-util 0.1 `src.single_class`/ExampleClass#a.
#                ^ reference  snapshot-util 0.1 `src.single_class`/ExampleClass#__init__().(a)
        self.b = b
#       ^^^^ reference  snapshot-util 0.1 `src.single_class`/ExampleClass#__init__().(self)
#            ^ reference[write]  snapshot-util 0.1 `src.single_class`/ExampleClass#b.
#                ^ reference  snapshot-util 0.1 `src.single_class`/ExampleClass#__init__().(b)
        self.c = "hello" + local_c
#       ^^^^ reference  snapshot-util 0.1 `src.single_class`/ExampleClass#__init__().(self)
#            ^ reference[write]  snapshot-util 0.1 `src.single_class`/ExampleClass#c.
#                          ^^^^^^^ reference local 0
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/single_function/.
// Matches scip-python's output, except that scip-python appends the parameter's name to the
// local symbol of a lambda parameter (`local 2(y)`), and resolves the keyword argument in
// `next_level()(a="wow")`, a call to the result of a call, which pyrefly does not.
#[test]
fn test_ported_single_function() {
    assert_index(
        &[(
            "src.single_function",
            r#"def my_cool_function(a: str) -> str:
    x = ", world"
    return a + x

def my_cool_function_2(a: str):
    x = ", world"
    return (lambda y: a + x + y)("oh no")

def next_level():
    return my_cool_function

my_cool_function("hello")
next_level()(a = "wow")
"#,
        )],
        r#"
# src/single_function.py
# < definition scip-python python snapshot-util 0.1 `src.single_function`/__init__:
def my_cool_function(a: str) -> str:
#   ^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 `src.single_function`/my_cool_function().
#    enclosing [0, 0, 2, 16]
#                    ^ definition  snapshot-util 0.1 `src.single_function`/my_cool_function().(a)
#                       ^^^ reference  python-stdlib 3.12 builtins/str#
#                               ^^^ reference  python-stdlib 3.12 builtins/str#
    x = ", world"
#   ^ definition local 0
    return a + x
#          ^ reference  snapshot-util 0.1 `src.single_function`/my_cool_function().(a)
#              ^ reference local 0

def my_cool_function_2(a: str):
#   ^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 `src.single_function`/my_cool_function_2().
#    enclosing [4, 0, 6, 41]
#                      ^ definition  snapshot-util 0.1 `src.single_function`/my_cool_function_2().(a)
#                         ^^^ reference  python-stdlib 3.12 builtins/str#
    x = ", world"
#   ^ definition local 1
    return (lambda y: a + x + y)("oh no")
#                  ^ definition local 2
#                     ^ reference  snapshot-util 0.1 `src.single_function`/my_cool_function_2().(a)
#                         ^ reference local 1
#                             ^ reference local 2

def next_level():
#   ^^^^^^^^^^ definition  snapshot-util 0.1 `src.single_function`/next_level().
#    enclosing [8, 0, 9, 27]
    return my_cool_function
#          ^^^^^^^^^^^^^^^^ reference  snapshot-util 0.1 `src.single_function`/my_cool_function().

my_cool_function("hello")
#^^^^^^^^^^^^^^^ reference  snapshot-util 0.1 `src.single_function`/my_cool_function().
next_level()(a = "wow")
#^^^^^^^^^ reference  snapshot-util 0.1 `src.single_function`/next_level().
"#,
    );
}

// Ported from scip-python's snapshot tests (MIT license):
// packages/pyright-scip/snapshots/input/unresolved_import/.
// Matches scip-python's output, except the unresolved `this_is_not_real`, which scip-python
// gives a local symbol and pyrefly no occurrence (GOAL.md §5.1, `unresolved-name` in
// scip-goal/ADJUDICATIONS.md).
#[test]
fn test_ported_unresolved_import() {
    assert_index(
        &[(
            "unresolved",
            r#"import this_is_not_real

print(this_is_not_real.x)
print(this_is_not_real.x)

"#,
        )],
        r#"
# unresolved.py
# < definition scip-python python snapshot-util 0.1 unresolved/__init__:
import this_is_not_real

print(this_is_not_real.x)
#^^^^ reference  python-stdlib 3.12 builtins/print().
print(this_is_not_real.x)
#^^^^ reference  python-stdlib 3.12 builtins/print().
"#,
    );
}
