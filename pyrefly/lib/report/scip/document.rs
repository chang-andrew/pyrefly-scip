/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The SCIP document of one checked file.

use std::borrow::Cow;
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
use pyrefly_python::module_path::ModulePathDetails;
use pyrefly_python::short_identifier::ShortIdentifier;
use pyrefly_python::symbol_kind::SymbolKind;
use pyrefly_python::sys_info::PythonVersion;
use ruff_python_ast::Identifier;
use ruff_text_size::TextRange;
use scip::types::Document;
use scip::types::Occurrence;
use scip::types::PositionEncoding;
use scip::types::Signature;
use scip::types::SymbolInformation;
use scip::types::SymbolRole;
use scip::types::symbol_information::Kind;

use crate::binding::binding::Key;
use crate::report::scip::definitions::Definitions;
use crate::report::scip::definitions::Detail;
use crate::report::scip::definitions::Site;
use crate::report::scip::definitions::SiteKind;
use crate::report::scip::definitions::SiteRole;
use crate::report::scip::definitions::SiteSymbol;
use crate::report::scip::references::References;
use crate::report::scip::symbols::Package;
use crate::report::scip::symbols::module_descriptors;
use crate::report::scip::symbols::module_namespace;
use crate::state::lsp::DefinitionMetadata;
use crate::state::lsp::FindDefinitionItemWithDocstring;
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
    stdlib: Package,
    /// The directory that document paths are relative to.
    root: PathBuf,
    definitions: DashMap<ModulePath, Arc<Definitions>>,
}

/// The symbol of an occurrence: a complete global symbol, or a local numbered within its
/// module.
#[derive(Debug, Clone)]
pub enum OccurrenceSymbol {
    Global(String),
    Local(u32),
}

/// An occurrence before its symbol is formatted: locals are numbered per document, in the
/// order of their first occurrence.
pub struct PendingOccurrence {
    pub range: TextRange,
    pub symbol: OccurrenceSymbol,
    pub roles: i32,
    pub enclosing_range: Vec<i32>,
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
    /// An indexer for the `project` package whose documents are relative to `root`, with
    /// standard library symbols in the `python-stdlib <python_version>` package.
    pub fn new(
        transaction: &'a Transaction<'a>,
        project: Package,
        python_version: PythonVersion,
        root: PathBuf,
    ) -> Self {
        Self {
            transaction,
            project,
            stdlib: Package::new(
                "python-stdlib".to_owned(),
                format!("{}.{}", python_version.major, python_version.minor),
            ),
            root,
            definitions: DashMap::new(),
        }
    }

    pub fn transaction(&self) -> &'a Transaction<'a> {
        self.transaction
    }

    /// The package of the symbols defined in `module`: the project for files under the
    /// project root, the standard library for bundled typeshed, and otherwise a package
    /// named after the module's top-level package.
    fn package(&self, module: &Module) -> Cow<'_, Package> {
        match module.path().details() {
            ModulePathDetails::BundledTypeshed(_) => Cow::Borrowed(&self.stdlib),
            ModulePathDetails::FileSystem(path)
            | ModulePathDetails::Memory(path)
            | ModulePathDetails::Namespace(path)
                if path.is_relative() || path.starts_with(&self.root) =>
            {
                Cow::Borrowed(&self.project)
            }
            _ => Cow::Owned(Package::new(
                module.name().first_component().to_string(),
                String::new(),
            )),
        }
    }

    /// The symbol of a definition that pyrefly resolved a reference in `handle` to: the
    /// module itself, or the binding site at the definition's range. A local symbol only
    /// resolves within its own module. A name bound to a module by an import resolves to
    /// the module with an empty range rather than with module metadata.
    pub fn resolve(
        &self,
        handle: &Handle,
        definition: &FindDefinitionItemWithDocstring,
    ) -> Option<OccurrenceSymbol> {
        let module = &definition.module;
        let package = self.package(module);
        if matches!(definition.metadata, DefinitionMetadata::Module)
            || (definition.metadata.symbol_kind() == Some(SymbolKind::Module)
                && definition.definition_range.is_empty())
        {
            return Some(OccurrenceSymbol::Global(
                package.symbol(&module_descriptors(module.name())),
            ));
        }
        let target = Handle::new(
            module.name(),
            module.path().dupe(),
            handle.sys_info().dupe(),
        );
        match &self
            .definitions(&target, module)
            .get(definition.definition_range)?
            .symbol
        {
            SiteSymbol::Global(descriptors) => {
                Some(OccurrenceSymbol::Global(package.symbol(descriptors)))
            }
            SiteSymbol::Local(id) => {
                (module.path() == handle.path()).then_some(OccurrenceSymbol::Local(*id))
            }
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

    /// The signature of the definition of `name` at `range`, as Python source: the `def` or
    /// `class` line with its decorators, or `name: type` for a variable. Types that are not
    /// written in the source come from pyrefly's inference.
    fn signature(
        &self,
        handle: &Handle,
        module: &Module,
        name: &str,
        range: TextRange,
        site: &Site,
    ) -> Option<String> {
        let answers = self.transaction.get_answers(handle);
        let type_of = |key: Key| {
            let answers = answers.as_ref()?;
            let bindings = answers.bindings();
            if !bindings.is_valid_key(&key) {
                return None;
            }
            answers.get_type_at(bindings.key_to_idx(&key))
        };
        let identifier = ShortIdentifier::new(&Identifier::new(name, range));
        match &site.detail {
            Some(Detail::Function {
                header,
                returns,
                is_async,
                ..
            }) => {
                let returns = returns.clone().or_else(|| {
                    let ty = type_of(Key::ReturnType(identifier))?;
                    let ty = if *is_async {
                        self.transaction
                            .ad_hoc_solve(handle, "scip_coroutine", |solver| {
                                solver.unwrap_coroutine(&ty)
                            })
                            .flatten()
                            .map_or(ty, |(_, _, returns)| returns)
                    } else {
                        ty
                    };
                    Some(ty.to_string())
                });
                Some(match returns {
                    Some(returns) => format!("{header} -> {returns}:"),
                    None => format!("{header}:"),
                })
            }
            Some(Detail::Class { header, .. }) => Some(header.clone()),
            Some(Detail::Annotation(annotation)) => Some(format!("{name}: {annotation}")),
            None => match site.kind {
                SiteKind::Variable | SiteKind::Field => Some(
                    match type_of(Key::Definition(identifier))
                        .or_else(|| self.transaction.get_type_trace(handle, range))
                    {
                        Some(ty) => format!("{name}: {ty}"),
                        None => name.to_owned(),
                    },
                ),
                _ => None,
            },
        }
        .filter(|_| module.code_at(range) == name)
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

        let definitions = self.definitions(handle, &module);
        let mut pending = Vec::new();
        for (range, site) in definitions.iter() {
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
                symbol: match &site.symbol {
                    SiteSymbol::Global(descriptors) => {
                        OccurrenceSymbol::Global(self.project.symbol(descriptors))
                    }
                    SiteSymbol::Local(id) => OccurrenceSymbol::Local(*id),
                },
                roles,
                enclosing_range,
            });
            if is_definition && let SiteSymbol::Global(descriptors) = &site.symbol {
                let name = module.code_at(*range);
                let mut documentation = Vec::new();
                let signature = self.signature(handle, &module, name, *range, site);
                if let Some(signature) = &signature {
                    documentation.push(format!("```python\n{signature}\n```"));
                }
                if let Some(Detail::Function { docstring, .. } | Detail::Class { docstring, .. }) =
                    &site.detail
                    && let Some(docstring) = docstring
                {
                    let docstring = Docstring(*docstring, module.dupe());
                    if docstring.has_content() {
                        documentation.push(docstring.resolve());
                    }
                }
                symbols.push(SymbolInformation {
                    symbol: self.project.symbol(descriptors),
                    documentation,
                    kind: scip_kind(site.kind).into(),
                    display_name: name.to_owned(),
                    signature_documentation: signature
                        .map(|text| Signature {
                            language: "python".to_owned(),
                            text,
                            ..Default::default()
                        })
                        .into(),
                    ..Default::default()
                });
            }
        }

        let ast = self.transaction.get_ast_or_parse_module(handle, &module);
        pending.extend(References::collect(
            self,
            handle,
            &module,
            &definitions,
            &ast,
        ));

        // Number locals in the order of their first occurrence, then sort occurrences by range
        // and symbol.
        pending.sort_by_key(|occurrence| (occurrence.range.start(), occurrence.range.end()));
        let mut locals = HashMap::new();
        let mut occurrences: Vec<(TextRange, Occurrence)> = pending
            .into_iter()
            .map(|occurrence| {
                let symbol = match occurrence.symbol {
                    OccurrenceSymbol::Global(symbol) => symbol,
                    OccurrenceSymbol::Local(id) => {
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
