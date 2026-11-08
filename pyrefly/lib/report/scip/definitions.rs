/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The symbol of every binding site in a module.
//!
//! A symbol is a pure function of its definition site, `(module, identifier range)`.
//! [`Definitions::new`] walks a module's AST once and records the symbol of every
//! identifier that binds a name, so that a definition occurrence and every reference that
//! resolves to one of its binding sites get the same symbol. It follows scip-python's
//! descriptors:
//!
//! - a class is `<owner>Name#`, where the owner is the enclosing module, class or function;
//! - a function is `<class>name().` when any class encloses it, even through other
//!   functions, and `<owner>name().` otherwise;
//! - a parameter is `<function>(name)` and a type parameter `<owner>[T]`;
//! - a variable of a module or class body is `<owner>name.`, even when the class is inside
//!   a function, and an instance attribute assigned through a method's `self` is
//!   `<class>name.`;
//! - every other name bound inside a function, lambda or comprehension is local.
//!
//! The first binding of a name in its scope is the symbol's definition; later bindings of
//! the same name are references. Instance attributes are defined by the class body when it
//! binds the name, and by their first assignment in source order otherwise. Like
//! scip-python, the throwaway name `_` has no sites.

use std::collections::HashMap;
use std::mem;

use itertools::Itertools;
use pyrefly_python::docstring::Docstring;
use ruff_python_ast::Comprehension;
use ruff_python_ast::Decorator;
use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprAttribute;
use ruff_python_ast::Identifier;
use ruff_python_ast::ModModule;
use ruff_python_ast::Parameters;
use ruff_python_ast::Pattern;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtClassDef;
use ruff_python_ast::StmtFunctionDef;
use ruff_python_ast::TypeParam;
use ruff_python_ast::TypeParams;
use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use ruff_python_ast::visitor::source_order::walk_expr;
use ruff_python_ast::visitor::source_order::walk_pattern;
use ruff_python_ast::visitor::source_order::walk_stmt;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use ruff_text_size::TextSize;
use starlark_map::small_set::SmallSet;

use crate::report::scip::symbols::escape_name;

/// The symbol of a binding site.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SiteSymbol {
    /// A global symbol, given by its descriptors (the part after the package).
    Global(String),
    /// A local symbol, numbered within its module.
    Local(u32),
}

/// What a binding site defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteKind {
    Class,
    Function,
    Method,
    Parameter,
    TypeParameter,
    /// A variable of a module body, or a local variable.
    Variable,
    /// A variable of a class body, or an instance attribute.
    Field,
}

/// How a site uses its symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteRole {
    /// The first binding of the name in its scope.
    Definition,
    /// A later binding of the name.
    Write,
    /// A `global` or `nonlocal` declaration, which names the symbol without binding it.
    Declaration,
}

/// The source text that documents a definition.
#[derive(Debug, Clone)]
pub enum Detail {
    /// A function: its decorators and `def` line without the return annotation and the
    /// final colon, its return annotation, and its docstring.
    Function {
        header: String,
        returns: Option<String>,
        is_async: bool,
        docstring: Option<TextRange>,
    },
    /// A class: its decorators and complete `class` line, and its docstring.
    Class {
        header: String,
        docstring: Option<TextRange>,
    },
    /// The annotation of an annotated variable.
    Annotation(String),
}

/// A site that binds or declares a name.
#[derive(Debug, Clone)]
pub struct Site {
    pub symbol: SiteSymbol,
    pub kind: SiteKind,
    pub role: SiteRole,
    /// The whole definition: a class or function from its keyword to the end of its body,
    /// or the statement that binds a global variable.
    pub enclosing_range: Option<TextRange>,
    pub detail: Option<Detail>,
}

/// `text` on one line, with every run of whitespace replaced by a single space.
fn one_line(text: &str) -> String {
    text.split_whitespace().join(" ")
}

/// The binding sites of one module, keyed by the range of their identifier.
#[derive(Debug, Default)]
pub struct Definitions {
    sites: HashMap<TextRange, Site>,
}

impl Definitions {
    /// Collects the binding sites of the module whose namespace descriptor is
    /// `module_namespace` (such as `` `a.b`/ ``) and whose source is `contents`.
    pub fn new(module_namespace: String, ast: &ModModule, contents: &str) -> Self {
        let mut collector = Collector {
            module_namespace,
            contents,
            scopes: vec![Scope::new(ScopeKind::Module)],
            statements: Vec::new(),
            sites: HashMap::new(),
            next_local: 0,
        };
        collector.visit_body(&ast.body);
        Self {
            sites: collector.sites,
        }
    }

    /// The binding site whose identifier is at `range`.
    pub fn get(&self, range: TextRange) -> Option<&Site> {
        self.sites.get(&range)
    }

    /// Every binding site, in source order.
    pub fn iter(&self) -> impl Iterator<Item = (&TextRange, &Site)> {
        self.sites
            .iter()
            .sorted_by_key(|(range, _)| (range.start(), range.end()))
    }
}

/// How a name was bound in a scope.
struct Binding {
    symbol: SiteSymbol,
    /// Whether a binding has been seen. A `global` or `nonlocal` declaration names the
    /// symbol of an enclosing scope before it may be bound there.
    bound: bool,
}

enum ScopeKind {
    Module,
    /// A class body, with the class's descriptors and the instance attributes that its
    /// methods assign through `self` (name, identifier range, statement range and
    /// annotation).
    Class {
        descriptors: String,
        attributes: Vec<(Name, TextRange, TextRange, Option<String>)>,
    },
    /// A function body, with the function's descriptors, the name of its `self` parameter
    /// when it is a method, and the names it declares `global` or `nonlocal`.
    Function {
        descriptors: String,
        self_name: Option<Name>,
        globals: SmallSet<Name>,
        nonlocals: SmallSet<Name>,
    },
    Lambda,
    Comprehension,
}

struct Scope {
    kind: ScopeKind,
    names: HashMap<Name, Binding>,
}

impl Scope {
    fn new(kind: ScopeKind) -> Self {
        Self {
            kind,
            names: HashMap::new(),
        }
    }
}

struct Collector<'a> {
    module_namespace: String,
    contents: &'a str,
    scopes: Vec<Scope>,
    /// The ranges of the statements being visited, innermost last.
    statements: Vec<TextRange>,
    sites: HashMap<TextRange, Site>,
    next_local: u32,
}

impl Collector<'_> {
    /// The descriptors that names defined in the innermost module, class or function are
    /// appended to.
    fn owner(&self) -> &str {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| match &scope.kind {
                ScopeKind::Module => Some(self.module_namespace.as_str()),
                ScopeKind::Class { descriptors, .. } | ScopeKind::Function { descriptors, .. } => {
                    Some(descriptors.as_str())
                }
                ScopeKind::Lambda | ScopeKind::Comprehension => None,
            })
            .expect("the module scope is always present")
    }

    /// The index of the scope where binding `name` in the innermost scope takes effect. A
    /// walrus skips comprehensions, and `global` and `nonlocal` declarations redirect the
    /// binding to the module or the enclosing function.
    fn binding_scope(&self, name: &Name, walrus: bool) -> usize {
        let mut index = self.scopes.len() - 1;
        if walrus {
            while matches!(self.scopes[index].kind, ScopeKind::Comprehension) {
                index -= 1;
            }
        }
        match &self.scopes[index].kind {
            ScopeKind::Function { globals, .. } if globals.contains(name) => 0,
            ScopeKind::Function { nonlocals, .. } if nonlocals.contains(name) => self.scopes
                [..index]
                .iter()
                .rposition(|s| matches!(s.kind, ScopeKind::Function { .. } | ScopeKind::Lambda))
                .unwrap_or(index),
            _ => index,
        }
    }

    fn new_local(&mut self) -> SiteSymbol {
        self.next_local += 1;
        SiteSymbol::Local(self.next_local - 1)
    }

    /// The symbol a variable named `name` gets when it is first bound in `scope`. Variables
    /// of a class body are global even when the class is inside a function.
    fn variable_symbol(&mut self, scope: usize, name: &str) -> SiteSymbol {
        match &self.scopes[scope].kind {
            ScopeKind::Module => {
                SiteSymbol::Global(format!("{}{}.", self.module_namespace, escape_name(name)))
            }
            ScopeKind::Class { descriptors, .. } => {
                SiteSymbol::Global(format!("{descriptors}{}.", escape_name(name)))
            }
            ScopeKind::Function { .. } | ScopeKind::Lambda | ScopeKind::Comprehension => {
                self.new_local()
            }
        }
    }

    /// Records a binding of `name` at `range` in `scope`, described by `site`. The first
    /// binding of a name gives it `site.symbol` and is its definition; later bindings reuse
    /// the name's symbol and are writes.
    fn bind(&mut self, scope: usize, name: &Name, range: TextRange, site: Site) -> SiteSymbol {
        let Site { symbol, .. } = &site;
        let symbol = symbol.clone();
        let (symbol, first) = match self.scopes[scope].names.get_mut(name) {
            Some(binding) => {
                let first = !binding.bound;
                binding.bound = true;
                (binding.symbol.clone(), first)
            }
            None => {
                self.scopes[scope].names.insert(
                    name.clone(),
                    Binding {
                        symbol: symbol.clone(),
                        bound: true,
                    },
                );
                (symbol, true)
            }
        };
        // scip-python leaves the throwaway name `_` out of the index.
        if name == "_" {
            return symbol;
        }
        self.sites.insert(
            range,
            Site {
                symbol: symbol.clone(),
                role: if first {
                    SiteRole::Definition
                } else {
                    SiteRole::Write
                },
                ..site
            },
        );
        symbol
    }

    /// A site template for [`Collector::bind`], whose role `bind` decides.
    fn site(
        symbol: SiteSymbol,
        kind: SiteKind,
        enclosing_range: Option<TextRange>,
        detail: Option<Detail>,
    ) -> Site {
        Site {
            symbol,
            kind,
            role: SiteRole::Definition,
            enclosing_range,
            detail,
        }
    }

    /// Binds a variable named `name` at `range`, from an assignment, a loop, a `with`, an
    /// `except`, a pattern or a walrus, with the annotation of an annotated assignment.
    fn bind_variable(
        &mut self,
        name: &Name,
        range: TextRange,
        walrus: bool,
        annotation: Option<String>,
    ) {
        let scope = self.binding_scope(name, walrus);
        let symbol = match self.scopes[scope].names.get(name) {
            Some(binding) => binding.symbol.clone(),
            None => self.variable_symbol(scope, name),
        };
        let kind = match self.scopes[scope].kind {
            ScopeKind::Class { .. } => SiteKind::Field,
            _ => SiteKind::Variable,
        };
        let enclosing_range = match symbol {
            SiteSymbol::Global(_) => self.statements.last().copied(),
            SiteSymbol::Local(_) => None,
        };
        self.bind(
            scope,
            name,
            range,
            Self::site(
                symbol,
                kind,
                enclosing_range,
                annotation.map(Detail::Annotation),
            ),
        );
    }

    /// Records a `global` or `nonlocal` name, which refers to the symbol of the module or of
    /// the enclosing function without binding it.
    fn declare(&mut self, identifier: &Identifier, global: bool) {
        let name = identifier.id();
        let current = self.scopes.len() - 1;
        if let ScopeKind::Function {
            globals, nonlocals, ..
        } = &mut self.scopes[current].kind
        {
            if global {
                globals.insert(name.clone());
            } else {
                nonlocals.insert(name.clone());
            }
        }
        let scope = self.binding_scope(name, false);
        let symbol = match self.scopes[scope].names.get(name) {
            Some(binding) => binding.symbol.clone(),
            None => {
                let symbol = self.variable_symbol(scope, name);
                self.scopes[scope].names.insert(
                    name.clone(),
                    Binding {
                        symbol: symbol.clone(),
                        bound: false,
                    },
                );
                symbol
            }
        };
        self.sites.insert(
            identifier.range,
            Site {
                symbol,
                kind: SiteKind::Variable,
                role: SiteRole::Declaration,
                enclosing_range: None,
                detail: None,
            },
        );
    }

    /// The range of a class or function definition from its keyword: decorators, and the
    /// whitespace and comments after them, are left out.
    fn definition_range(&self, range: TextRange, decorators: &[Decorator]) -> TextRange {
        let Some(last) = decorators.last() else {
            return range;
        };
        let rest = &self.contents[TextRange::new(last.range.end(), range.end())];
        let mut in_comment = false;
        let offset = rest
            .char_indices()
            .find(|&(_, c)| {
                if in_comment {
                    in_comment = c != '\n';
                    false
                } else if c == '#' {
                    in_comment = true;
                    false
                } else {
                    !c.is_whitespace() && c != '\\'
                }
            })
            .map_or(0, |(offset, _)| offset);
        TextRange::new(
            last.range.end() + TextSize::try_from(offset).unwrap(),
            range.end(),
        )
    }

    /// The source text of an optional node, on one line.
    fn text_of(&self, node: Option<&impl Ranged>) -> String {
        node.map_or_else(String::new, |node| one_line(&self.contents[node.range()]))
    }

    /// The decorators of a definition, one per line, as written.
    fn decorator_lines(&self, decorators: &[Decorator]) -> String {
        decorators
            .iter()
            .map(|decorator| format!("{}\n", &self.contents[decorator.range]))
            .collect()
    }

    /// A function's decorators and `def` line, with one parameter per line as scip-python
    /// writes them, without the return annotation and the final colon.
    fn function_header(&self, function: &StmtFunctionDef) -> String {
        let parameters = &function.parameters;
        let text = |range: TextRange| one_line(&self.contents[range]);
        let mut items: Vec<String> = parameters
            .posonlyargs
            .iter()
            .map(|p| text(p.range))
            .collect();
        if !items.is_empty() {
            items.push("/".to_owned());
        }
        items.extend(parameters.args.iter().map(|p| text(p.range)));
        match &parameters.vararg {
            Some(vararg) => items.push(text(vararg.range)),
            None if !parameters.kwonlyargs.is_empty() => items.push("*".to_owned()),
            None => {}
        }
        items.extend(parameters.kwonlyargs.iter().map(|p| text(p.range)));
        if let Some(kwarg) = &parameters.kwarg {
            items.push(text(kwarg.range));
        }
        let parameters = if items.is_empty() {
            "()".to_owned()
        } else {
            format!("(\n  {}\n)", items.join(",\n  "))
        };
        format!(
            "{}{} {}{}{parameters}",
            self.decorator_lines(&function.decorator_list),
            if function.is_async {
                "async def"
            } else {
                "def"
            },
            function.name,
            self.text_of(function.type_params.as_deref()),
        )
    }

    fn bind_type_params(&mut self, type_params: Option<&TypeParams>, descriptors: &str) {
        let scope = self.scopes.len() - 1;
        for type_param in type_params.into_iter().flat_map(|t| &t.type_params) {
            let name = match type_param {
                TypeParam::TypeVar(t) => &t.name,
                TypeParam::TypeVarTuple(t) => &t.name,
                TypeParam::ParamSpec(t) => &t.name,
            };
            let symbol =
                SiteSymbol::Global(format!("{descriptors}[{}]", escape_name(name.as_str())));
            self.bind(
                scope,
                name.id(),
                name.range,
                Self::site(symbol, SiteKind::TypeParameter, None, None),
            );
        }
    }

    fn bind_parameters(&mut self, parameters: &Parameters, descriptors: Option<&str>) {
        let scope = self.scopes.len() - 1;
        for parameter in parameters {
            let name = &parameter.name();
            let symbol = match descriptors {
                Some(descriptors) => {
                    SiteSymbol::Global(format!("{descriptors}({})", escape_name(name.as_str())))
                }
                None => self.new_local(),
            };
            self.bind(
                scope,
                name.id(),
                name.range,
                Self::site(symbol, SiteKind::Parameter, None, None),
            );
        }
    }

    /// Visits the parts of a parameter list that are evaluated in the enclosing scope:
    /// annotations and defaults.
    fn visit_parameter_expressions(&mut self, parameters: &Parameters) {
        for parameter in parameters {
            if let Some(annotation) = parameter.annotation() {
                self.visit_expr(annotation);
            }
            if let Some(default) = parameter.default() {
                self.visit_expr(default);
            }
        }
    }

    fn visit_function(&mut self, function: &StmtFunctionDef) {
        for decorator in &function.decorator_list {
            self.visit_decorator(decorator);
        }
        self.visit_parameter_expressions(&function.parameters);
        if let Some(returns) = &function.returns {
            self.visit_expr(returns);
        }
        let name = escape_name(function.name.as_str());
        let enclosing_class = self
            .scopes
            .iter()
            .rev()
            .find_map(|scope| match &scope.kind {
                ScopeKind::Class { descriptors, .. } => Some(descriptors.as_str()),
                _ => None,
            });
        let descriptors = match enclosing_class {
            Some(class) => format!("{class}{name}()."),
            None => format!("{}{name}().", self.owner()),
        };
        let current = self.scopes.len() - 1;
        let is_method = matches!(self.scopes[current].kind, ScopeKind::Class { .. });
        let scope = self.binding_scope(function.name.id(), false);
        let enclosing_range = self.definition_range(function.range, &function.decorator_list);
        let detail = Detail::Function {
            header: self.function_header(function),
            returns: function
                .returns
                .as_ref()
                .map(|returns| one_line(&self.contents[returns.range()])),
            is_async: function.is_async,
            docstring: Docstring::range_from_stmts(&function.body),
        };
        self.bind(
            scope,
            function.name.id(),
            function.name.range,
            Self::site(
                SiteSymbol::Global(descriptors.clone()),
                if is_method {
                    SiteKind::Method
                } else {
                    SiteKind::Function
                },
                Some(enclosing_range),
                Some(detail),
            ),
        );
        let is_static = function.decorator_list.iter().any(|decorator| {
            matches!(&decorator.expression, Expr::Name(name) if name.id == "staticmethod")
        });
        let self_name = (is_method && !is_static)
            .then(|| {
                function
                    .parameters
                    .posonlyargs
                    .iter()
                    .chain(&function.parameters.args)
                    .next()
                    .map(|parameter| parameter.parameter.name.id.clone())
            })
            .flatten();
        self.scopes.push(Scope::new(ScopeKind::Function {
            descriptors: descriptors.clone(),
            self_name,
            globals: SmallSet::new(),
            nonlocals: SmallSet::new(),
        }));
        self.bind_type_params(function.type_params.as_deref(), &descriptors);
        self.bind_parameters(&function.parameters, Some(&descriptors));
        self.visit_body(&function.body);
        self.scopes.pop();
    }

    fn visit_class(&mut self, class: &StmtClassDef) {
        for decorator in &class.decorator_list {
            self.visit_decorator(decorator);
        }
        if let Some(arguments) = &class.arguments {
            self.visit_arguments(arguments);
        }
        let descriptors = format!("{}{}#", self.owner(), escape_name(class.name.as_str()));
        let scope = self.binding_scope(class.name.id(), false);
        let enclosing_range = self.definition_range(class.range, &class.decorator_list);
        let detail = Detail::Class {
            header: format!(
                "{}class {}{}{}:",
                self.decorator_lines(&class.decorator_list),
                class.name,
                self.text_of(class.type_params.as_deref()),
                self.text_of(class.arguments.as_deref()),
            ),
            docstring: Docstring::range_from_stmts(&class.body),
        };
        self.bind(
            scope,
            class.name.id(),
            class.name.range,
            Self::site(
                SiteSymbol::Global(descriptors.clone()),
                SiteKind::Class,
                Some(enclosing_range),
                Some(detail),
            ),
        );
        self.scopes.push(Scope::new(ScopeKind::Class {
            descriptors: descriptors.clone(),
            attributes: Vec::new(),
        }));
        self.bind_type_params(class.type_params.as_deref(), &descriptors);
        self.visit_body(&class.body);
        let class_scope = self.scopes.len() - 1;
        let ScopeKind::Class { attributes, .. } = &mut self.scopes[class_scope].kind else {
            unreachable!("the class scope was pushed above");
        };
        for (name, range, statement, annotation) in mem::take(attributes) {
            let symbol = SiteSymbol::Global(format!("{descriptors}{}.", escape_name(&name)));
            self.bind(
                class_scope,
                &name,
                range,
                Self::site(
                    symbol,
                    SiteKind::Field,
                    Some(statement),
                    annotation.map(Detail::Annotation),
                ),
            );
        }
        self.scopes.pop();
    }

    /// Records `<self>.name = ...` in a method as an instance attribute of its class.
    fn record_attribute(&mut self, attribute: &ExprAttribute, annotation: Option<String>) {
        let Expr::Name(receiver) = attribute.value.as_ref() else {
            return;
        };
        let [.., class, method] = self.scopes.as_mut_slice() else {
            return;
        };
        if let (
            ScopeKind::Class { attributes, .. },
            ScopeKind::Function {
                self_name: Some(self_name),
                ..
            },
        ) = (&mut class.kind, &method.kind)
            && receiver.id == *self_name
        {
            attributes.push((
                attribute.attr.id.clone(),
                attribute.attr.range,
                *self
                    .statements
                    .last()
                    .expect("an assignment is a statement"),
                annotation,
            ));
        }
    }
}

impl<'a> SourceOrderVisitor<'a> for Collector<'_> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        self.statements.push(stmt.range());
        match stmt {
            Stmt::FunctionDef(function) => self.visit_function(function),
            Stmt::ClassDef(class) => self.visit_class(class),
            Stmt::Global(global) => {
                for name in &global.names {
                    self.declare(name, true);
                }
            }
            Stmt::Nonlocal(nonlocal) => {
                for name in &nonlocal.names {
                    self.declare(name, false);
                }
            }
            Stmt::AnnAssign(assign) => {
                self.visit_annotation(&assign.annotation);
                if let Some(value) = &assign.value {
                    self.visit_expr(value);
                }
                let annotation = one_line(&self.contents[assign.annotation.range()]);
                match assign.target.as_ref() {
                    Expr::Name(name) => {
                        self.bind_variable(&name.id, name.range, false, Some(annotation));
                    }
                    Expr::Attribute(attribute) => {
                        self.record_attribute(attribute, Some(annotation));
                        self.visit_expr(&attribute.value);
                    }
                    target => self.visit_expr(target),
                }
            }
            // Imports bind names, but references to them resolve to what they import.
            Stmt::Import(_) | Stmt::ImportFrom(_) => {}
            _ => walk_stmt(self, stmt),
        }
        self.statements.pop();
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Name(name) if name.ctx.is_store() => {
                self.bind_variable(&name.id, name.range, false, None);
            }
            Expr::Named(named) => {
                self.visit_expr(&named.value);
                if let Expr::Name(name) = named.target.as_ref() {
                    self.bind_variable(&name.id, name.range, true, None);
                }
            }
            Expr::Attribute(attribute) if attribute.ctx.is_store() => {
                self.record_attribute(attribute, None);
                walk_expr(self, expr);
            }
            Expr::Lambda(lambda) => {
                if let Some(parameters) = &lambda.parameters {
                    self.visit_parameter_expressions(parameters);
                }
                self.scopes.push(Scope::new(ScopeKind::Lambda));
                if let Some(parameters) = &lambda.parameters {
                    self.bind_parameters(parameters, None);
                }
                self.visit_expr(&lambda.body);
                self.scopes.pop();
            }
            Expr::ListComp(_) | Expr::SetComp(_) | Expr::DictComp(_) | Expr::Generator(_) => {
                let (generators, elements): (&[Comprehension], Vec<&Expr>) = match expr {
                    Expr::ListComp(c) => (&c.generators, vec![&c.elt]),
                    Expr::SetComp(c) => (&c.generators, vec![&c.elt]),
                    Expr::Generator(c) => (&c.generators, vec![&c.elt]),
                    Expr::DictComp(c) => (
                        &c.generators,
                        c.key.as_deref().into_iter().chain([&*c.value]).collect(),
                    ),
                    _ => unreachable!("matched by the outer pattern"),
                };
                self.scopes.push(Scope::new(ScopeKind::Comprehension));
                for generator in generators {
                    self.visit_expr(&generator.iter);
                    self.visit_expr(&generator.target);
                    for condition in &generator.ifs {
                        self.visit_expr(condition);
                    }
                }
                for element in elements {
                    self.visit_expr(element);
                }
                self.scopes.pop();
            }
            _ => walk_expr(self, expr),
        }
    }

    fn visit_except_handler(&mut self, handler: &'a ExceptHandler) {
        let ExceptHandler::ExceptHandler(handler) = handler;
        if let Some(type_) = &handler.type_ {
            self.visit_expr(type_);
        }
        if let Some(name) = &handler.name {
            self.bind_variable(name.id(), name.range, false, None);
        }
        self.visit_body(&handler.body);
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        let name = match pattern {
            Pattern::MatchAs(p) => p.name.as_ref(),
            Pattern::MatchStar(p) => p.name.as_ref(),
            Pattern::MatchMapping(p) => p.rest.as_ref(),
            _ => None,
        };
        walk_pattern(self, pattern);
        if let Some(name) = name {
            self.bind_variable(name.id(), name.range, false, None);
        }
    }
}
