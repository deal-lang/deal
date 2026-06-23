//! `textDocument/completion` handler (Plan 03-04 Task 2).
//!
//! Returns two completion-item groups:
//!
//! 1. The 16 DEAL element-definition keywords (SD-1/SD-18/SD-20/SD-21) as
//!    snippet-format items so the user can press Tab through declaration
//!    scaffolds.
//! 2. The workspace types — every PathString in the in-memory `Index`
//!    whose terminal segment looks like a type name (starts uppercase).
//!    The detail field carries the fully-qualified path so the user can
//!    disambiguate name collisions across packages.
//!
//! Trigger characters per RESEARCH §1: `.`, `:`, `<`. Backend declares
//! these in CompletionOptions so the client invokes completion on
//! qualified-name typing, type-annotation `:`, and relationship-operator
//! `<<…>>` typing.
//!
//! The element-keyword set is the source of truth shared with Plan 03-01's
//! `vscode-deal/snippets/deal.json` (SD-1). The names + descriptions
//! here mirror the snippet labels so the editor IntelliSense surface is
//! visually consistent across the snippet UI and the LSP completion UI.

use std::collections::HashSet;

use ropey::Rope;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Documentation,
    InsertTextFormat, MarkupContent, MarkupKind, Position, Range, TextEdit,
};

use crate::documents::Documents;
use crate::index::Index;

/// The 16 DEAL element-definition keywords from SD-1 / SD-18 / SD-20 / SD-21.
///
/// Each entry is `(prefix, snippet-body, human-description)`. Snippet
/// bodies use `${N:placeholder}` for tab-stops and `${0}` for the final
/// cursor landing per LSP SnippetTextFormat (LSP §3.18).
///
/// This is the source of truth shared with `vscode-deal/snippets/deal.json`
/// (SD-1) — keep the two in sync. The set mirrors the element keywords in
/// `deal/src/keywords.zig` and the AST `*_def` node kinds in `deal/src/ast.zig`.
pub const ELEMENT_KEYWORDS: &[(&str, &str, &str)] = &[
    (
        "part def",
        "part def ${1:Name} {\n\t${0}\n}",
        "Declare a part definition (SD-1).",
    ),
    (
        "port def",
        "port def ${1:Name} {\n\tdirection: ${2|in,out,inout|};\n\t${0}\n}",
        "Declare a port definition with direction (SD-1, SD-8).",
    ),
    (
        "action def",
        "action def ${1:Name}(${2:params}) {\n\t${0}\n}",
        "Declare an action definition (SD-1).",
    ),
    (
        "state def",
        "state def ${1:Name} {\n\t${0}\n}",
        "Declare a state definition (SD-1).",
    ),
    (
        "requirement def",
        "requirement def ${1:ReqId} {\n\tdoc: \"${2:Requirement statement}\";\n\t${0}\n}",
        "Declare a requirement definition (SD-1, SD-18).",
    ),
    (
        "constraint def",
        "constraint def ${1:Name} {\n\t${0}\n}",
        "Declare a constraint definition (SD-1).",
    ),
    (
        "calc def",
        "calc def ${1:Name}(${2:params}) : ${3:Type} {\n\treturn ${0};\n}",
        "Declare a calculation definition (SD-21).",
    ),
    (
        "attribute def",
        "attribute def ${1:Name} : ${2:Type};",
        "Declare an attribute definition (SD-1, SD-2).",
    ),
    (
        "item def",
        "item def ${1:Name} {\n\t${0}\n}",
        "Declare an item definition (SD-1).",
    ),
    (
        "interface def",
        "interface def ${1:Name} {\n\t${0}\n}",
        "Declare an interface definition (SD-1).",
    ),
    (
        "connection def",
        "connection def ${1:Name} {\n\t${0}\n}",
        "Declare a connection definition (SD-1, CS-5).",
    ),
    (
        "flow def",
        "flow def ${1:Name} {\n\tsource: ${2:src};\n\ttarget: ${3:dst};\n}",
        "Declare a flow definition (SD-1, CS-5).",
    ),
    (
        "allocation def",
        "allocation def ${1:Name} {\n\t${0}\n}",
        "Declare an allocation definition (SD-1).",
    ),
    (
        "need def",
        "need def ${1:Name} {\n\t${0}\n}",
        "Declare a need definition (SD-18).",
    ),
    (
        "use case def",
        "use case def ${1:Name} {\n\tsubject ${2:subject};\n\t${0}\n}",
        "Declare a use-case definition (SD-20).",
    ),
    (
        "actor def",
        "actor def ${1:Name};",
        "Declare an actor definition (SD-18b).",
    ),
];

/// The line index where a new `import` statement should be inserted: just after
/// the last existing `import` line, else just after the `package …;` line, else
/// the top of the file (ADR-0004 P5 WS-D auto-import).
fn import_insert_line(rope: &Rope) -> u32 {
    let mut last_import: Option<usize> = None;
    let mut package_line: Option<usize> = None;
    for (i, line) in rope.lines().enumerate() {
        let s = line.to_string();
        let t = s.trim_start();
        if t.starts_with("import ") {
            last_import = Some(i);
        } else if package_line.is_none() && t.starts_with("package ") {
            package_line = Some(i);
        }
    }
    let line = last_import.map(|i| i + 1).or(package_line.map(|i| i + 1)).unwrap_or(0);
    line as u32
}

/// Implementation of `textDocument/completion`.
///
/// Tolerates a not-yet-populated index gracefully — the element-keyword items
/// are always returned even before eager_parse completes.
///
/// ADR-0004 P5 WS-D: completion is **additive**. Visible types (own package or
/// already imported) insert the bare name; types from an un-imported package are
/// offered as auto-import items carrying an `additionalTextEdits` that inserts
/// the `import <pkg>.{<Name>};` line — matching the vscode-java / TS norm for a
/// strict-import language (you complete it, the import is added for you).
pub async fn handle_completion(
    documents: &Documents,
    index: &Index,
    params: CompletionParams,
) -> LspResult<Option<CompletionResponse>> {
    let uri = &params.text_document_position.text_document.uri;

    // The editing file's own package + already-imported packages + where a new
    // import line would go — read from its already-parsed handle + buffer (no
    // re-parse). Absent (file not analyzed) → no auto-import edits (plain items).
    let mut own_package = String::new();
    let mut imported: HashSet<String> = HashSet::new();
    let mut insert_line: Option<u32> = None;
    if let Some(handle) = documents.get_handle(uri) {
        let env_bytes = {
            let guard = handle.lock().await;
            deal_ffi::safe::index_json(&guard).unwrap_or_default()
        };
        if let Ok(env) = serde_json::from_slice::<serde_json::Value>(&env_bytes) {
            if let Some(p) = env.get("package").and_then(|v| v.as_str()) {
                own_package = p.to_string();
            }
            if let Some(arr) = env.get("imports_graph").and_then(|v| v.as_array()) {
                for e in arr {
                    if let Some(ip) = e.get("import_path").and_then(|v| v.as_str()) {
                        imported.insert(ip.to_string());
                    }
                }
            }
        }
        if let Some(rope) = documents.get_buffer(uri) {
            insert_line = Some(import_insert_line(&rope));
        }
    }

    let mut items: Vec<CompletionItem> = Vec::with_capacity(64);

    // Group 1: element keywords as snippet items.
    for (label, body, desc) in ELEMENT_KEYWORDS {
        items.push(CompletionItem {
            label: (*label).to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("DEAL element keyword".to_string()),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: (*desc).to_string(),
            })),
            insert_text: Some((*body).to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }

    // Group 2: workspace + dependency types — every PathString in the index
    // whose terminal segment starts uppercase (DEAL type-name convention).
    for (path, _loc) in index.snapshot() {
        let (pkg, name) = match path.rsplit_once('.') {
            Some((p, n)) => (p, n),
            None => ("", path.as_str()),
        };
        let is_type_name = name
            .chars()
            .next()
            .map(|c| c.is_ascii_uppercase())
            .unwrap_or(false);
        if !is_type_name {
            continue;
        }

        // Visible if it's the file's own package or an imported package; only
        // a genuinely un-imported package gets an auto-import edit (and only
        // when we know where to insert it).
        let in_scope = pkg.is_empty() || pkg == own_package || imported.contains(pkg);
        let additional_text_edits = if in_scope {
            None
        } else {
            insert_line.map(|line| {
                vec![TextEdit {
                    range: Range::new(Position::new(line, 0), Position::new(line, 0)),
                    new_text: format!("import {pkg}.{{{name}}};\n"),
                }]
            })
        };
        let detail = if additional_text_edits.is_some() {
            format!("{path}  (auto-import)")
        } else {
            path.clone()
        };

        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(detail),
            insert_text: Some(name.to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            additional_text_edits,
            ..Default::default()
        });
    }

    Ok(Some(CompletionResponse::Array(items)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;
    use tower_lsp::lsp_types::{
        PartialResultParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
        WorkDoneProgressParams,
    };

    fn dummy_params() -> CompletionParams {
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::parse("file:///x.deal").unwrap(),
                },
                position: Position::new(0, 0),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        }
    }

    #[tokio::test]
    async fn keywords_always_returned_even_with_empty_index() {
        let idx = Index::new();
        let resp = handle_completion(&Documents::new(), &idx, dummy_params())
            .await
            .unwrap()
            .unwrap();
        let CompletionResponse::Array(items) = resp else {
            panic!("expected Array variant");
        };
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        for (kw, _, _) in ELEMENT_KEYWORDS {
            assert!(
                labels.contains(kw),
                "missing element keyword {kw} in completion result"
            );
        }
        // All keywords are SNIPPET-formatted.
        let kw_items: Vec<&CompletionItem> = items
            .iter()
            .filter(|i| i.kind == Some(CompletionItemKind::KEYWORD))
            .collect();
        assert_eq!(kw_items.len(), ELEMENT_KEYWORDS.len());
        for item in &kw_items {
            assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
        }
    }

    #[tokio::test]
    async fn workspace_types_appear_with_class_kind() {
        let idx = Index::new();
        let url = Url::parse("file:///a.deal").unwrap();
        let env = br#"{"v":1,"elements":{
            "vehicle.battery.BatteryPack":{"id":"x","kind":"part_def","source_file":"a","span":[0,10]},
            "vehicle.battery.BatteryCell":{"id":"x","kind":"part_def","source_file":"a","span":[0,10]},
            "vehicle.battery.lowercaseHelper":{"id":"x","kind":"attribute_usage","source_file":"a","span":[0,10]}
        }}"#;
        idx.update_from_envelope(&url, env, &Rope::from_str("part def X {} "));
        let resp = handle_completion(&Documents::new(), &idx, dummy_params())
            .await
            .unwrap()
            .unwrap();
        let CompletionResponse::Array(items) = resp else {
            panic!("expected Array variant");
        };
        let class_items: Vec<&CompletionItem> = items
            .iter()
            .filter(|i| i.kind == Some(CompletionItemKind::CLASS))
            .collect();
        // BatteryPack + BatteryCell appear; lowercaseHelper filtered out.
        let class_labels: Vec<&str> = class_items.iter().map(|i| i.label.as_str()).collect();
        assert!(class_labels.contains(&"BatteryPack"));
        assert!(class_labels.contains(&"BatteryCell"));
        assert!(!class_labels.contains(&"lowercaseHelper"));
        // Detail field carries the fully-qualified path for disambiguation.
        let pack = class_items
            .iter()
            .find(|i| i.label == "BatteryPack")
            .unwrap();
        assert_eq!(pack.detail.as_deref(), Some("vehicle.battery.BatteryPack"));
    }

    fn params_at(uri: &Url) -> CompletionParams {
        CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position::new(2, 0),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        }
    }

    /// WS-D: a type from an un-imported package is offered with an auto-import
    /// `additionalTextEdits`; a same-package type is offered plain.
    #[tokio::test]
    async fn unimported_type_offers_auto_import_edit() {
        let docs = Documents::new();
        let index = Index::new();
        let app = Url::parse("file:///app.deal").unwrap();
        // The editing file: package app, no imports, one local def.
        docs.open_silent(
            app.clone(),
            "package app;\npart def Local {\n    public (\n        attribute n : Real [1];\n    )\n}\n"
                .to_string(),
            &index,
        )
        .await
        .unwrap();
        // A type from an un-imported package, injected into the index.
        let other = Url::parse("file:///other.deal").unwrap();
        let env = br#"{"v":1,"elements":{
            "deal.std.units.Mass":{"id":"x","kind":"part_def","source_file":"o","span":[0,10]}
        }}"#;
        index.update_from_envelope(&other, env, &Rope::from_str("package deal.std.units;"));

        let resp = handle_completion(&docs, &index, params_at(&app))
            .await
            .unwrap()
            .unwrap();
        let CompletionResponse::Array(items) = resp else {
            panic!("expected Array variant");
        };

        let mass = items
            .iter()
            .find(|i| i.label == "Mass")
            .expect("Mass offered");
        let edits = mass
            .additional_text_edits
            .as_ref()
            .expect("unimported Mass carries an auto-import edit");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "import deal.std.units.{Mass};\n");
        // Inserted after the `package app;` line (line 0) → line 1.
        assert_eq!(edits[0].range.start.line, 1);

        let local = items
            .iter()
            .find(|i| i.label == "Local")
            .expect("Local offered");
        assert!(
            local.additional_text_edits.is_none(),
            "own-package type must not carry an import edit"
        );
    }

    #[test]
    fn element_keyword_count_matches_sd1() {
        // 16 element-definition keywords (SD-1/SD-18/SD-20/SD-21), matching the
        // *_def node kinds in deal/src/ast.zig. Catches accidental drift.
        assert_eq!(ELEMENT_KEYWORDS.len(), 16);
    }

    #[test]
    fn element_keywords_cover_all_ast_def_kinds() {
        // Every element keyword must be present; in particular the five added
        // in P1 (calc/allocation/need/use case/actor) must not regress.
        let labels: Vec<&str> = ELEMENT_KEYWORDS.iter().map(|(k, _, _)| *k).collect();
        for kw in [
            "calc def",
            "allocation def",
            "need def",
            "use case def",
            "actor def",
        ] {
            assert!(labels.contains(&kw), "missing element keyword {kw}");
        }
    }
}
