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
use crate::test::util::mk_multi_file_state_assert_no_errors;

/// Renders the document of every file in `files`, in order, each under a `# <path>` header.
fn index(files: &[(&'static str, &str)], show_docs: bool) -> String {
    let (handles, state) = mk_multi_file_state_assert_no_errors(files, Require::Everything);
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
#              ^^^^^ reference  python-stdlib 3.12 functools/cache().
    def method(self, *args, **kwargs):
#       ^^^^^^ definition  snapshot-util 0.1 defs/Base#method().
#        enclosing [20, 4, 21, 26]
#              ^^^^ definition  snapshot-util 0.1 defs/Base#method().(self)
#                     ^^^^ definition  snapshot-util 0.1 defs/Base#method().(args)
#                             ^^^^^^ definition  snapshot-util 0.1 defs/Base#method().(kwargs)
        return lambda x: x
#                     ^ definition local 2

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
