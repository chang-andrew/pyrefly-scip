/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The reference occurrences of one module.
//!
//! Every name, attribute, keyword argument and import that is not itself a binding site is
//! resolved with pyrefly's go-to-definition, following imports to the original definition,
//! and gets the symbol of the binding site it resolves to (see [`Indexer::resolve`]). A
//! reference that does not resolve, such as one into an unresolved import, has no
//! occurrence.

use pyrefly_build::handle::Handle;
use pyrefly_python::module_name::ModuleName;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprAttribute;
use ruff_python_ast::ExprName;
use ruff_python_ast::Identifier;
use ruff_python_ast::Keyword;
use ruff_python_ast::ModModule;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtImport;
use ruff_python_ast::StmtImportFrom;
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use ruff_python_ast::visitor::source_order::walk_expr;
use ruff_python_ast::visitor::source_order::walk_stmt;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use ruff_text_size::TextSize;
use scip::types::SymbolRole;

use crate::report::scip::definitions::Definitions;
use crate::report::scip::document::Indexer;
use crate::report::scip::document::PendingOccurrence;
use crate::state::lsp::FindDefinitionItemWithDocstring;
use crate::state::lsp::FindPreference;
use crate::state::lsp::resolve_relative_module_name;

pub struct References<'i, 'a> {
    indexer: &'i Indexer<'a>,
    handle: &'i Handle,
    definitions: &'i Definitions,
    occurrences: Vec<PendingOccurrence>,
}

impl<'i, 'a> References<'i, 'a> {
    /// The reference occurrences of the module `handle`, whose binding sites are
    /// `definitions` and whose AST is `ast`.
    pub fn collect(
        indexer: &'i Indexer<'a>,
        handle: &'i Handle,
        definitions: &'i Definitions,
        ast: &ModModule,
    ) -> Vec<PendingOccurrence> {
        let mut references = Self {
            indexer,
            handle,
            definitions,
            occurrences: Vec::new(),
        };
        references.visit_body(&ast.body);
        references.occurrences
    }

    /// Records a reference at `range` to `definition`, with the `roles` bitset.
    fn push(
        &mut self,
        range: TextRange,
        definition: Option<FindDefinitionItemWithDocstring>,
        roles: i32,
    ) {
        if let Some(symbol) =
            definition.and_then(|definition| self.indexer.resolve(self.handle, &definition))
        {
            self.occurrences.push(PendingOccurrence {
                range,
                symbol,
                roles,
                enclosing_range: Vec::new(),
            });
        }
    }

    /// The definition of the identifier at `position`, found the way go-to-definition finds
    /// it there.
    fn definition_at(&self, position: TextSize) -> Option<FindDefinitionItemWithDocstring> {
        self.indexer
            .transaction()
            .find_definition(self.handle, position, FindPreference::default())
            .ok()
            .map(|definitions| definitions.first().clone())
    }

    fn module_definition(&self, module: ModuleName) -> Option<FindDefinitionItemWithDocstring> {
        self.indexer
            .transaction()
            .find_definition_for_imported_module(self.handle, module, FindPreference::default())
            .ok()
            .flatten()
    }

    fn visit_name(&mut self, name: &ExprName) {
        if name.id == "_" {
            return;
        }
        let definition = self
            .indexer
            .transaction()
            .find_definition_for_name_use(
                self.handle,
                &Identifier::new(name.id.clone(), name.range),
                FindPreference::default(),
            )
            .ok()
            .flatten();
        let role = if name.ctx.is_del() {
            SymbolRole::WriteAccess
        } else {
            SymbolRole::ReadAccess
        };
        self.push(name.range, definition, role as i32);
    }

    fn visit_attribute(&mut self, attribute: &ExprAttribute) {
        if self.definitions.get(attribute.attr.range).is_some() {
            return;
        }
        let definition = self
            .indexer
            .transaction()
            .find_definition_for_attribute(
                self.handle,
                attribute.value.range(),
                attribute.attr.id(),
                FindPreference::default(),
            )
            .ok()
            .map(|definitions| definitions.first().clone());
        let role = if attribute.ctx.is_load() {
            SymbolRole::ReadAccess
        } else {
            SymbolRole::WriteAccess
        };
        self.push(attribute.attr.range, definition, role as i32);
    }

    /// `import a.b.c` refers to module `a.b.c` across the whole dotted name, and
    /// `import a.b as c` also refers to it at `c`.
    fn visit_import(&mut self, import: &StmtImport) {
        for alias in &import.names {
            let definition = self.module_definition(ModuleName::from_str(alias.name.as_str()));
            if let Some(asname) = &alias.asname {
                self.push(asname.range, definition.clone(), SymbolRole::Import as i32);
            }
            self.push(alias.name.range, definition, SymbolRole::Import as i32);
        }
    }

    /// `from .a import b as c` refers to module `.a` across its dots and name, and to what
    /// `b` imports across `b as c`, as scip-python's occurrences do.
    fn visit_import_from(&mut self, import: &StmtImportFrom) {
        if let Some(module) = &import.module {
            let name = resolve_relative_module_name(
                self.handle,
                ModuleName::from_str(module.as_str()),
                import.level,
            );
            let range = TextRange::new(
                module.range.start() - TextSize::from(import.level),
                module.range.end(),
            );
            self.push(
                range,
                self.module_definition(name),
                SymbolRole::Import as i32,
            );
        }
        for alias in &import.names {
            if alias.name.as_str() != "*" {
                let definition = self.definition_at(alias.name.range.start());
                self.push(alias.range, definition, SymbolRole::Import as i32);
            }
        }
    }
}

impl<'v> SourceOrderVisitor<'v> for References<'_, '_> {
    fn visit_stmt(&mut self, stmt: &'v Stmt) {
        match stmt {
            Stmt::Import(import) => self.visit_import(import),
            Stmt::ImportFrom(import) => self.visit_import_from(import),
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'v Expr) {
        match expr {
            Expr::Name(name) if !name.ctx.is_store() => self.visit_name(name),
            Expr::Attribute(attribute) => {
                self.visit_expr(&attribute.value);
                self.visit_attribute(attribute);
            }
            _ => walk_expr(self, expr),
        }
    }

    /// A keyword argument refers to the parameter it passes, without an access role.
    fn visit_keyword(&mut self, keyword: &'v Keyword) {
        if let Some(arg) = &keyword.arg {
            let definition = self.definition_at(arg.range.start());
            self.push(arg.range, definition, 0);
        }
        self.visit_expr(&keyword.value);
    }
}
