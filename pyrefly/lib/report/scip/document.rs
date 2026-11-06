/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The SCIP document of one checked file.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use dupe::Dupe;
use pyrefly_build::handle::Handle;
use pyrefly_python::docstring::Docstring;
use pyrefly_python::module::Module;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use ruff_text_size::TextRange;
use scip::types::Document;
use scip::types::Occurrence;
use scip::types::PositionEncoding;
use scip::types::SymbolInformation;
use scip::types::SymbolRole;
use scip::types::symbol_information::Kind;

use crate::report::scip::definitions::Definitions;
use crate::report::scip::definitions::SiteKind;
use crate::report::scip::definitions::SiteRole;
use crate::report::scip::definitions::SiteSymbol;
use crate::report::scip::symbols::Package;
use crate::report::scip::symbols::module_descriptors;
use crate::report::scip::symbols::module_namespace;
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

/// Builds documents, sharing the binding sites of each module between them.
pub struct Indexer<'a> {
    transaction: &'a Transaction<'a>,
    project: Package,
    /// The directory that document paths are relative to.
    root: PathBuf,
    definitions: DashMap<ModulePath, Arc<Definitions>>,
}

/// An occurrence before its symbol is formatted: locals are numbered per document, in the
/// order of their first occurrence.
struct PendingOccurrence {
    range: TextRange,
    symbol: SiteSymbol,
    roles: i32,
    enclosing_range: Vec<i32>,
}

fn to_scip_range(module: &Module, range: TextRange) -> Vec<i32> {
    let range = module.to_lsp_range(range);
    let (start, end) = (range.start, range.end);
    if start.line == end.line {
        vec![
            start.line as i32,
            start.character as i32,
            end.character as i32,
        ]
    } else {
        vec![
            start.line as i32,
            start.character as i32,
            end.line as i32,
            end.character as i32,
        ]
    }
}

fn scip_kind(kind: SiteKind) -> Kind {
    match kind {
        SiteKind::Class => Kind::Class,
        SiteKind::Function => Kind::Function,
        SiteKind::Method => Kind::Method,
        SiteKind::Parameter => Kind::Parameter,
        SiteKind::TypeParameter => Kind::TypeParameter,
        SiteKind::Variable => Kind::Variable,
        SiteKind::Field => Kind::Field,
    }
}

impl<'a> Indexer<'a> {
    pub fn new(transaction: &'a Transaction<'a>, project: Package, root: PathBuf) -> Self {
        Self {
            transaction,
            project,
            root,
            definitions: DashMap::new(),
        }
    }

    /// The name of the module `handle` in its symbols. A file that cannot be imported, such
    /// as `0001_initial.py`, is named after its path from the search path entry that holds
    /// it, or from the project root, as scip-python does.
    fn module_name(&self, handle: &Handle, module: &Module) -> ModuleName {
        if module.name() != ModuleName::unknown() {
            return module.name();
        }
        let path = handle.path().as_path();
        let config = self.transaction.get_config(handle);
        config
            .iter()
            .flat_map(|config| config.search_path())
            .chain([&self.root])
            .find_map(|root| ModuleName::from_relative_path(path.strip_prefix(root).ok()?).ok())
            .unwrap_or_else(ModuleName::unknown)
    }

    /// The binding sites of the module `handle`, computed once per module.
    fn definitions(&self, handle: &Handle, module: &Module) -> Arc<Definitions> {
        if let Some(definitions) = self.definitions.get(handle.path()) {
            return definitions.dupe();
        }
        let ast = self.transaction.get_ast_or_parse_module(handle, module);
        let definitions = Arc::new(Definitions::new(
            module_namespace(self.module_name(handle, module)),
            &ast,
            module.contents(),
        ));
        self.definitions
            .entry(handle.path().dupe())
            .or_insert(definitions)
            .dupe()
    }

    /// The document for `handle`, whose path relative to the project root is `relative_path`.
    pub fn document(&self, handle: &Handle, relative_path: String) -> Document {
        let module = self
            .transaction
            .get_module_info(handle)
            .expect("a checked module has module info");
        let module_name = self.module_name(handle, &module);
        let module_symbol = self.project.symbol(&module_descriptors(module_name));
        let mut documentation = vec![format!("(module) {module_name}")];
        if let Some(range) = self.transaction.get_module_docstring_range(handle) {
            let docstring = Docstring(range, module.dupe());
            if docstring.has_content() {
                documentation.push(docstring.resolve());
            }
        }
        let mut symbols = vec![SymbolInformation {
            symbol: module_symbol.clone(),
            documentation,
            kind: Kind::Module.into(),
            display_name: module_name.to_string(),
            ..Default::default()
        }];

        let mut pending = Vec::new();
        for (range, site) in self.definitions(handle, &module).iter() {
            let is_definition = site.role == SiteRole::Definition;
            let roles = match site.role {
                SiteRole::Definition => SymbolRole::Definition as i32,
                SiteRole::Write => SymbolRole::WriteAccess as i32,
                SiteRole::Declaration => 0,
            };
            let enclosing_range = match site.enclosing_range {
                Some(enclosing) if is_definition => to_scip_range(&module, enclosing),
                _ => Vec::new(),
            };
            pending.push(PendingOccurrence {
                range: *range,
                symbol: site.symbol.clone(),
                roles,
                enclosing_range,
            });
            if is_definition && let SiteSymbol::Global(descriptors) = &site.symbol {
                symbols.push(SymbolInformation {
                    symbol: self.project.symbol(descriptors),
                    kind: scip_kind(site.kind).into(),
                    display_name: module.code_at(*range).to_owned(),
                    ..Default::default()
                });
            }
        }

        // Number locals in the order of their first occurrence, then sort occurrences by range
        // and symbol.
        pending.sort_by_key(|occurrence| (occurrence.range.start(), occurrence.range.end()));
        let mut locals = HashMap::new();
        let mut occurrences: Vec<(TextRange, Occurrence)> = pending
            .into_iter()
            .map(|occurrence| {
                let symbol = match occurrence.symbol {
                    SiteSymbol::Global(descriptors) => self.project.symbol(&descriptors),
                    SiteSymbol::Local(id) => {
                        let next = locals.len();
                        format!("local {}", locals.entry(id).or_insert(next))
                    }
                };
                let scip_occurrence = Occurrence {
                    range: to_scip_range(&module, occurrence.range),
                    symbol,
                    symbol_roles: occurrence.roles,
                    enclosing_range: occurrence.enclosing_range,
                    ..Default::default()
                };
                (occurrence.range, scip_occurrence)
            })
            .collect();
        occurrences.sort_by(|(a_range, a), (b_range, b)| {
            (a_range.start(), a_range.end(), &a.symbol).cmp(&(
                b_range.start(),
                b_range.end(),
                &b.symbol,
            ))
        });
        let occurrences = [Occurrence {
            range: vec![0, 0, 0],
            symbol: module_symbol,
            symbol_roles: SymbolRole::Definition as i32,
            ..Default::default()
        }]
        .into_iter()
        .chain(occurrences.into_iter().map(|(_, occurrence)| occurrence))
        .collect();
        symbols.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        // A name redefined in the same module, such as a function in two branches of an
        // `if`, gives its symbol several definitions; keep one symbol information.
        symbols.dedup_by(|a, b| a.symbol == b.symbol);

        Document {
            language: "python".to_owned(),
            relative_path,
            occurrences,
            symbols,
            position_encoding: PositionEncoding::UTF16CodeUnitOffsetFromLineStart.into(),
            ..Default::default()
        }
    }
}
