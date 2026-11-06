/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Renders a SCIP document in the style of scip-python's snapshot tests, for review in
//! tests: each source line is followed by one `#   ^^^ <role> <symbol>` line per occurrence
//! that starts on it, with the `scip-python python` prefix of global symbols left out.

use std::collections::BTreeMap;

use scip::types::Document;
use scip::types::Occurrence;
use scip::types::SymbolInformation;
use scip::types::SymbolRole;

/// A symbol without the scheme and manager that every global symbol shares.
fn short(symbol: &str) -> &str {
    symbol.strip_prefix("scip-python python").unwrap_or(symbol)
}

/// The role column: `definition` or `reference`, with the write and import roles in
/// brackets. Read access is not shown, since every other reference has it.
fn role(occurrence: &Occurrence) -> String {
    let roles = occurrence.symbol_roles;
    let mut out = if roles & SymbolRole::Definition as i32 != 0 {
        "definition".to_owned()
    } else {
        "reference".to_owned()
    };
    for (bit, name) in [
        (SymbolRole::WriteAccess, "write"),
        (SymbolRole::Import, "import"),
    ] {
        if roles & bit as i32 != 0 {
            out.push_str(&format!("[{name}]"));
        }
    }
    out
}

/// Renders `document` over `source`. With `show_docs`, the symbol information of each
/// definition (kind, display name, documentation and signature) is printed under it;
/// relationships and enclosing ranges are always printed.
pub fn render(source: &str, document: &Document, show_docs: bool) -> String {
    let infos: BTreeMap<&str, &SymbolInformation> = document
        .symbols
        .iter()
        .map(|info| (info.symbol.as_str(), info))
        .collect();
    let mut by_line: BTreeMap<i32, Vec<&Occurrence>> = BTreeMap::new();
    for occurrence in &document.occurrences {
        by_line
            .entry(occurrence.range[0])
            .or_default()
            .push(occurrence);
    }
    let mut out = String::new();
    for (line_number, line) in source.split('\n').enumerate() {
        let occurrences = by_line
            .get(&(line_number as i32))
            .map_or(&[][..], Vec::as_slice);
        // An empty range at the start of the file, such as the module definition, goes
        // above the first line.
        let (before, after): (Vec<_>, Vec<_>) =
            occurrences.iter().partition(|o| o.range[..] == [0, 0, 0]);
        for occurrence in before {
            out.push_str(&format!("# < {} {}\n", role(occurrence), occurrence.symbol));
            render_info(&mut out, "#", &infos, occurrence, show_docs);
        }
        out.push_str(line);
        out.push('\n');
        for occurrence in after {
            let range = &occurrence.range;
            let start = range[1] as usize;
            let end = if range.len() == 3 {
                range[2] as usize
            } else if range[2] == range[0] {
                range[3] as usize
            } else {
                line.encode_utf16().count()
            };
            let width = end.saturating_sub(start).max(1);
            let prefix = if start == 0 {
                format!("#{}", "^".repeat((width - 1).max(1)))
            } else {
                format!("#{}{}", " ".repeat(start - 1), "^".repeat(width))
            };
            let indent = format!("#{}", " ".repeat(start.saturating_sub(1)));
            out.push_str(&format!(
                "{prefix} {} {}",
                role(occurrence),
                short(&occurrence.symbol)
            ));
            if range.len() == 4 && range[2] != range[0] {
                out.push_str(&format!(" (to {}:{})", range[2], range[3]));
            }
            out.push('\n');
            if !occurrence.enclosing_range.is_empty() {
                out.push_str(&format!(
                    "{indent} enclosing {:?}\n",
                    occurrence.enclosing_range
                ));
            }
            render_info(&mut out, &indent, &infos, occurrence, show_docs);
        }
    }
    out
}

fn render_info(
    out: &mut String,
    indent: &str,
    infos: &BTreeMap<&str, &SymbolInformation>,
    occurrence: &Occurrence,
    show_docs: bool,
) {
    if occurrence.symbol_roles & SymbolRole::Definition as i32 == 0 {
        return;
    }
    let Some(info) = infos.get(occurrence.symbol.as_str()) else {
        return;
    };
    for relationship in &info.relationships {
        if relationship.is_implementation {
            out.push_str(&format!(
                "{indent} relationship implementation {}\n",
                short(&relationship.symbol)
            ));
        }
    }
    if !show_docs {
        return;
    }
    out.push_str(&format!(
        "{indent} kind {:?} display_name {}\n",
        info.kind.enum_value_or_default(),
        info.display_name
    ));
    if let Some(signature) = info.signature_documentation.as_ref() {
        for (i, line) in signature.text.lines().enumerate() {
            let label = if i == 0 { "signature" } else { "         " };
            out.push_str(&format!("{indent} {label} {line}\n"));
        }
    }
    for documentation in &info.documentation {
        for (i, line) in documentation.lines().enumerate() {
            let label = if i == 0 {
                "documentation"
            } else {
                "             "
            };
            out.push_str(&format!("{indent} {label} {line}\n"));
        }
    }
}
