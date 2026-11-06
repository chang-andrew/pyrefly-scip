/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::path::Path;
use std::path::PathBuf;

use pretty_assertions::assert_eq;

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

LIMIT: int = 3
#^^^^ definition  snapshot-util 0.1 defs/LIMIT.
# enclosing [3, 0, 14]
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
        self.name = name
#            ^^^^ definition  snapshot-util 0.1 defs/Base#name.
#             enclosing [12, 8, 24]
        self.kind = name
#            ^^^^ reference[write]  snapshot-util 0.1 defs/Base#kind.
        local = [n for n in name]
#       ^^^^^ definition local 0
#                      ^ definition local 1

        def helper(value):
#           ^^^^^^ definition  snapshot-util 0.1 defs/Base#helper().
#            enclosing [16, 8, 17, 24]
#                  ^^^^^ definition  snapshot-util 0.1 defs/Base#helper().(value)
            return value

    @functools.cache
    def method(self, *args, **kwargs):
#       ^^^^^^ definition  snapshot-util 0.1 defs/Base#method().
#        enclosing [20, 4, 21, 26]
#              ^^^^ definition  snapshot-util 0.1 defs/Base#method().(self)
#                     ^^^^ definition  snapshot-util 0.1 defs/Base#method().(args)
#                             ^^^^^^ definition  snapshot-util 0.1 defs/Base#method().(kwargs)
        return lambda x: x
#                     ^ definition local 2

    @staticmethod
    def static(x):
#       ^^^^^^ definition  snapshot-util 0.1 defs/Base#static().
#        enclosing [24, 4, 25, 18]
#              ^ definition  snapshot-util 0.1 defs/Base#static().(x)
        x.attr = 1

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
    total = b
#   ^^^^^ reference[write] local 3
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
#       ^^^^^ definition local 4
        def method(self):
#           ^^^^^^ definition  snapshot-util 0.1 defs/outer().Local#method().
#            enclosing [38, 8, 39, 26]
#                  ^^^^ definition  snapshot-util 0.1 defs/outer().Local#method().(self)
            self.value = 1
#                ^^^^^ definition  snapshot-util 0.1 defs/outer().Local#value.
#                 enclosing [39, 12, 26]
    return inner

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
#                       ^^^^^ definition  snapshot-util 0.1 defs/error.
#                        enclosing [47, 4, 50, 12]
        pass

def generic[T](x: T) -> T:
#   ^^^^^^^ definition  snapshot-util 0.1 defs/generic().
#    enclosing [52, 0, 53, 12]
#           ^ definition  snapshot-util 0.1 defs/generic().[T]
#              ^ definition  snapshot-util 0.1 defs/generic().(x)
    return x
"#,
    );
}
