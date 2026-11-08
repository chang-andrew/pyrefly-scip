/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! SCIP symbol strings in scip-python's grammar: `scip-python python <package> <version>`
//! followed by descriptors, such as `` `a.b`/C#method(). `` for a method of class `C` in
//! module `a.b`.

use std::borrow::Cow;

use pyrefly_python::module_name::ModuleName;

/// The scheme and package manager of every global symbol.
const SCHEME_AND_MANAGER: &str = "scip-python python";

/// The package (name and version) a global symbol belongs to, formatted as the two symbol
/// tokens that follow the scheme and manager.
#[derive(Debug, Clone)]
pub struct Package(String);

impl Package {
    pub fn new(name: String, version: String) -> Self {
        Self(format!(
            "{} {}",
            package_token(&name),
            package_token(&version)
        ))
    }

    /// The symbol for the descriptors `descriptors` in this package.
    pub fn symbol(&self, descriptors: &str) -> String {
        format!("{SCHEME_AND_MANAGER} {} {descriptors}", self.0)
    }
}

/// A package name or version as a symbol token: empty becomes `.` and spaces are doubled.
fn package_token(token: &str) -> Cow<'_, str> {
    if token.is_empty() {
        Cow::Borrowed(".")
    } else if token.contains(' ') {
        Cow::Owned(token.replace(' ', "  "))
    } else {
        Cow::Borrowed(token)
    }
}

/// A descriptor name, in backticks unless it is a simple identifier (ASCII letters, digits,
/// `_`, `$`, `+` and `-`).
pub fn escape_name(name: &str) -> Cow<'_, str> {
    if !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'+' | b'-'))
    {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(format!("`{}`", name.replace('`', "``")))
    }
}

/// The namespace descriptor of a module, such as `` `a.b`/ `` or `a/`.
pub fn module_namespace(module: ModuleName) -> String {
    format!("{}/", escape_name(module.as_str()))
}

/// The descriptors of a module's own symbol, such as `` `a.b`/__init__: ``.
pub fn module_descriptors(module: ModuleName) -> String {
    format!("{}__init__:", module_namespace(module))
}
