/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The SCIP document of one checked file.

use std::path::Path;

use pyrefly_build::handle::Handle;
use pyrefly_python::docstring::Docstring;
use pyrefly_python::module_path::ModulePath;
use scip::types::Document;
use scip::types::Occurrence;
use scip::types::PositionEncoding;
use scip::types::SymbolInformation;
use scip::types::SymbolRole;
use scip::types::symbol_information::Kind;

use crate::report::scip::symbols::Package;
use crate::report::scip::symbols::module_descriptors;
use crate::state::state::Transaction;

/// The path of a module relative to `root`, with `/` separators on every platform. A path
/// outside `root` is kept as it is.
pub fn relative_path(path: &ModulePath, root: &Path) -> String {
    let path = path.as_path();
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The document for `handle`, whose path relative to the project root is `relative_path`.
pub fn document(
    transaction: &Transaction,
    handle: &Handle,
    relative_path: String,
    project: &Package,
) -> Document {
    let module = handle.module();
    let module_symbol = project.symbol(&module_descriptors(module));
    let mut documentation = vec![format!("(module) {module}")];
    if let Some(info) = transaction.get_module_info(handle)
        && let Some(range) = transaction.get_module_docstring_range(handle)
    {
        let docstring = Docstring(range, info);
        if docstring.has_content() {
            documentation.push(docstring.resolve());
        }
    }
    Document {
        language: "python".to_owned(),
        relative_path,
        occurrences: vec![Occurrence {
            range: vec![0, 0, 0],
            symbol: module_symbol.clone(),
            symbol_roles: SymbolRole::Definition as i32,
            ..Default::default()
        }],
        symbols: vec![SymbolInformation {
            symbol: module_symbol,
            documentation,
            kind: Kind::Module.into(),
            display_name: module.to_string(),
            ..Default::default()
        }],
        position_encoding: PositionEncoding::UTF16CodeUnitOffsetFromLineStart.into(),
        ..Default::default()
    }
}
