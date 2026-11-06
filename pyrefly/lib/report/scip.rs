/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Writes a [SCIP](https://github.com/sourcegraph/scip) index of the checked files.
//!
//! Symbols follow the conventions of scip-python, so that an index written here can stand in
//! for one written by scip-python and cross-index links keep working. In particular, a symbol
//! string is a pure function of its definition site (see [`symbols`]), and both the
//! definition occurrence and every reference compute it the same way.
//!
//! The index is streamed: the metadata is written first, then each document as its own
//! length-delimited `Index.documents` field. Concatenated repeated fields form a valid
//! `Index`, so the whole index is never held in memory.

mod document;
#[cfg(test)]
mod snapshot;
mod symbols;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::env::current_dir;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Context;
use lsp_types::Uri;
use parking_lot::Mutex;
use protobuf::Message;
use pyrefly_build::handle::Handle;
use scip::types::Metadata;
use scip::types::ProtocolVersion;
use scip::types::TextEncoding;
use scip::types::ToolInfo;

use crate::VERSION;
use crate::report::scip::document::document;
use crate::report::scip::document::relative_path;
use crate::report::scip::symbols::Package;
use crate::state::state::Transaction;

/// The protobuf tag of `Index.metadata` (field 1, length-delimited).
const METADATA_TAG: u8 = 0x0a;
/// The protobuf tag of each `Index.documents` entry (field 2, length-delimited).
const DOCUMENT_TAG: u8 = 0x12;

/// Writes the SCIP index of `handles` to `path`. Document paths, and the default project
/// name, come from the current working directory.
pub fn write_scip(
    transaction: &Transaction,
    handles: &[Handle],
    path: &Path,
    project_name: Option<&str>,
    project_version: &str,
) -> anyhow::Result<()> {
    let root = current_dir()?;
    let project = Package::new(
        project_name.map_or_else(
            || {
                root.file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
            },
            str::to_owned,
        ),
        project_version.to_owned(),
    );
    let mut handles: Vec<(String, &Handle)> = handles
        .iter()
        .map(|handle| (relative_path(handle.path(), &root), handle))
        .collect();
    handles.sort_by(|a, b| a.0.cmp(&b.0));

    let mut writer = BufWriter::new(
        File::create(path).with_context(|| format!("creating `{}`", path.display()))?,
    );
    writer.write_all(&[METADATA_TAG])?;
    metadata(&root).write_length_delimited_to_writer(&mut writer)?;

    // Workers claim documents in sorted order and hand each encoded document to `Pending`,
    // which writes every document that is next in line. Documents finish roughly in the
    // order they were claimed, so few wait in `ready` at any time.
    struct Pending<W> {
        next: usize,
        ready: BTreeMap<usize, Vec<u8>>,
        writer: W,
        result: anyhow::Result<()>,
    }
    let pending = Mutex::new(Pending {
        next: 0,
        ready: BTreeMap::new(),
        writer,
        result: Ok(()),
    });
    let claimed = AtomicUsize::new(0);
    transaction.thread_pool().spawn_many(|| {
        loop {
            let index = claimed.fetch_add(1, Ordering::Relaxed);
            let Some((relative_path, handle)) = handles.get(index) else {
                return;
            };
            let bytes = document(transaction, handle, relative_path.clone(), &project)
                .write_length_delimited_to_bytes()
                .expect("encoding a SCIP document to memory cannot fail");
            let mut guard = pending.lock();
            let pending = &mut *guard;
            pending.ready.insert(index, bytes);
            while let Some(bytes) = pending.ready.remove(&pending.next) {
                pending.next += 1;
                if pending.result.is_ok() {
                    pending.result = pending
                        .writer
                        .write_all(&[DOCUMENT_TAG])
                        .and_then(|()| pending.writer.write_all(&bytes))
                        .map_err(anyhow::Error::from);
                }
            }
        }
    });
    let pending = pending.into_inner();
    pending.result?;
    pending.writer.into_inner()?.sync_all()?;
    Ok(())
}

fn metadata(root: &Path) -> Metadata {
    Metadata {
        version: ProtocolVersion::UnspecifiedProtocolVersion.into(),
        tool_info: Some(ToolInfo {
            name: "pyrefly".to_owned(),
            version: VERSION.to_owned(),
            ..Default::default()
        })
        .into(),
        project_root: Uri::from_directory_path(root)
            .map_or_else(|()| String::new(), |uri| uri.to_string()),
        text_document_encoding: TextEncoding::UTF8.into(),
        ..Default::default()
    }
}
