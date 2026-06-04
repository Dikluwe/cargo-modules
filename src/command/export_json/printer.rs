// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Faithful JSON serialization of the dependency graph.
//!
//! The output schema is:
//!
//! ```json
//! {
//!   "crate": "<crate name>",
//!   "nodes": [ { "id": 0, "path": "...", "name": "...", "kind": "...", "visibility": "...", "position": { "file": "...", "start_line": 1, "end_line": 9 }, ... } ],
//!   "edges": [ { "from": "...", "id_from": 0, "to": "...", "id_to": 1, "relation": "owns" | "uses", "uses_kind": "reference" | "import" } ]
//! }
//! ```
//!
//! Each node may carry a `position`: the source `file` (absolute path) and the
//! 1-based, inclusive `start_line`/`end_line` of its declaration. It is absent
//! for items without an `.rs` source (e.g. builtin types); macro-generated
//! items map to their call site. This is additive — consumers of the previous
//! schema keep working.
//!
//! `uses_kind` is an additive subtype of a `uses` edge: `"reference"` for a
//! direct use of a type in a signature/field, or `"import"` for an `use`
//! declaration attributed to a module. It is absent for `owns` edges (and the
//! `relation` field itself is unchanged: still `"owns"` / `"uses"`).
//!
//! The `id` field is the petgraph node index of the underlying graph, exposed
//! so downstream tooling can distinguish nodes that happen to share the same
//! `path` (this can happen e.g. when a derive-expanded item and an inherent
//! item collide). `id` is unique within a single emitted JSON; stability
//! across invocations is not guaranteed.
//!
//! On top of the structural fields, each node carries a *semantic descriptor*
//! derived from the `hir::ModuleDef` it wraps (see
//! `lab/investiga-descritor-fork/relatorio.md`). The default descriptor
//! (always emitted) adds `trait`, `trait_ref`, the `is_const`/`is_async`/
//! `is_unsafe` modifiers, `cfg`, `macro_kind` and `is_non_exhaustive`. The
//! richer (and more expensive) `signature` and `generics` fields are emitted
//! only behind `--rich`.
//!
//! All descriptor fields are additive: the pre-existing fields — including the
//! `kind` string — are emitted unchanged, so consumers of the previous schema
//! keep working.

use ra_ap_hir::{self as hir, AsAssocItem as _, DisplayTarget, HirDisplay as _, MacroKind};
use ra_ap_ide::{self as ide, Edition};
use ra_ap_vfs::{self as vfs};

use petgraph::visit::{IntoNodeReferences, NodeRef};
use serde::Serialize;

use crate::{
    analyzer,
    graph::{Edge, Graph, Node, Relationship},
    item::{ItemCfgAttr, ItemVisibility},
};

use super::options::Options;

#[derive(Serialize)]
struct JsonGraph {
    #[serde(rename = "crate")]
    krate: String,
    nodes: Vec<JsonNode>,
    edges: Vec<JsonEdge>,
}

#[derive(Serialize)]
struct JsonNode {
    id: usize,
    path: String,
    name: String,
    kind: String,
    visibility: String,

    // --- Group A: default descriptor (always emitted when meaningful) ---
    /// Name of the trait this item declares or implements (`None` for items
    /// that are not trait-associated, e.g. free functions or inherent impls).
    #[serde(rename = "trait", skip_serializing_if = "Option::is_none")]
    trait_name: Option<String>,
    /// Full trait reference *with* generic arguments, e.g. `From<Abs>`. Only
    /// present for items of a trait `impl`; distinguishes same-named traits
    /// (`From<X>` vs `From<Y>`) that `trait` alone cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    trait_ref: Option<String>,
    /// Function/trait modifiers, emitted only when `true` (absence ⇒ `false`).
    /// The `kind` string still carries them too, unchanged.
    #[serde(skip_serializing_if = "is_false")]
    is_const: bool,
    #[serde(skip_serializing_if = "is_false")]
    is_async: bool,
    #[serde(skip_serializing_if = "is_false")]
    is_unsafe: bool,
    /// Structured `#[cfg(...)]` expressions gating the item (empty ⇒ omitted).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cfg: Vec<ItemCfgAttr>,
    /// For macro nodes: `macro_rules!`, `derive`, `attr` or `fn-like`.
    #[serde(skip_serializing_if = "Option::is_none")]
    macro_kind: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    is_non_exhaustive: bool,

    /// Source position: the file and 1-based, inclusive line range where the
    /// item is declared. Absent for items without an `.rs` source on disk
    /// (e.g. builtin types). Macro-generated items map to their call site.
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<JsonSpan>,

    // --- Group B: rich descriptor (only with `--rich`) ---
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generics: Option<Vec<JsonGeneric>>,
}

#[derive(Serialize)]
struct JsonSpan {
    /// Absolute path to the source file (as `analyzer::module_file` resolves
    /// it). Relativizing to the crate root, if needed, is left to consumers.
    file: String,
    /// 1-based line of the item's first character.
    start_line: u32,
    /// 1-based line of the item's last character.
    end_line: u32,
}

#[derive(Serialize)]
struct JsonGeneric {
    name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bounds: Vec<String>,
}

#[derive(Serialize)]
struct JsonEdge {
    from: String,
    id_from: usize,
    to: String,
    id_to: usize,
    relation: &'static str,
    /// Subtype of a `uses` edge: `"reference"` (direct use in a
    /// signature/field) or `"import"` (an `use` declaration attributed to the
    /// module). Absent for `owns`.
    #[serde(skip_serializing_if = "Option::is_none")]
    uses_kind: Option<&'static str>,
}

pub struct Printer<'a> {
    options: &'a Options,
    krate: hir::Crate,
    db: &'a ide::RootDatabase,
    vfs: &'a vfs::Vfs,
    edition: Edition,
}

impl<'a> Printer<'a> {
    pub fn new(
        options: &'a Options,
        krate: hir::Crate,
        db: &'a ide::RootDatabase,
        vfs: &'a vfs::Vfs,
        edition: Edition,
    ) -> Self {
        Self {
            options,
            krate,
            db,
            vfs,
            edition,
        }
    }

    pub fn to_json(&self, graph: &Graph<Node, Edge>) -> anyhow::Result<String> {
        let krate_name = analyzer::crate_name(self.krate, self.db);

        let mut nodes: Vec<JsonNode> = graph
            .node_references()
            .map(|node_ref| self.node_to_json(node_ref.id().index(), node_ref.weight()))
            .collect();
        nodes.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.id.cmp(&b.id)));

        let mut edges: Vec<JsonEdge> = graph
            .edge_indices()
            .map(|edge_idx| {
                let (source_idx, target_idx) = graph.edge_endpoints(edge_idx).unwrap();
                let edge = &graph[edge_idx];
                JsonEdge {
                    from: graph[source_idx].display_path(self.db, self.edition),
                    id_from: source_idx.index(),
                    to: graph[target_idx].display_path(self.db, self.edition),
                    id_to: target_idx.index(),
                    relation: relation_name(edge),
                    uses_kind: uses_kind_name(edge),
                }
            })
            .collect();
        edges.sort_by(|a, b| {
            a.from
                .cmp(&b.from)
                .then_with(|| a.to.cmp(&b.to))
                .then_with(|| a.relation.cmp(b.relation))
                .then_with(|| a.uses_kind.cmp(&b.uses_kind))
                .then_with(|| a.id_from.cmp(&b.id_from))
                .then_with(|| a.id_to.cmp(&b.id_to))
        });

        let payload = JsonGraph {
            krate: krate_name,
            nodes,
            edges,
        };

        let serialized = if self.options.compact {
            serde_json::to_string(&payload)?
        } else {
            serde_json::to_string_pretty(&payload)?
        };

        Ok(serialized)
    }

    fn node_to_json(&self, id: usize, node: &Node) -> JsonNode {
        let path = node.display_path(self.db, self.edition);
        let name = node.display_name(self.db, self.edition);
        let kind = node.kind_display_name(self.db, self.edition).to_string();
        let visibility = visibility_string(&node.visibility(self.db, self.edition));

        let (trait_name, trait_ref) = self.descriptor_trait(node);
        let (is_const, is_async, is_unsafe) = self.descriptor_modifiers(node);
        let cfg = analyzer::cfg_attrs(node.hir, self.db);
        let macro_kind = self.descriptor_macro_kind(node);
        let is_non_exhaustive = node
            .hir
            .attrs(self.db)
            .map(|attrs| attrs.is_non_exhaustive())
            .unwrap_or(false);

        let position = self.node_position(node);

        let (signature, generics) = if self.options.rich {
            (self.descriptor_signature(node), self.descriptor_generics(node))
        } else {
            (None, None)
        };

        JsonNode {
            id,
            path,
            name,
            kind,
            visibility,
            trait_name,
            trait_ref,
            is_const,
            is_async,
            is_unsafe,
            cfg,
            macro_kind,
            is_non_exhaustive,
            position,
            signature,
            generics,
        }
    }

    fn display_target(&self) -> DisplayTarget {
        self.krate.to_display_target(self.db)
    }

    /// `(trait_name, trait_ref)` for a trait-associated item.
    ///
    /// `trait_name` is the bare trait name (resolves `Display` vs `Debug`).
    /// `trait_ref` is the concrete trait reference with generic arguments
    /// (resolves `From<X>` vs `From<Y>`) and exists only for `impl` items.
    fn descriptor_trait(&self, node: &Node) -> (Option<String>, Option<String>) {
        let Some(assoc_item) = node.hir.as_assoc_item(self.db) else {
            return (None, None);
        };

        let trait_name = assoc_item
            .container_or_implemented_trait(self.db)
            .map(|trait_hir| trait_hir.name(self.db).display(self.db, self.edition).to_string());

        let trait_ref = match assoc_item.container(self.db) {
            hir::AssocItemContainer::Impl(impl_hir) => impl_hir
                .trait_ref(self.db)
                .map(|trait_ref| trait_ref.display(self.db, self.display_target()).to_string()),
            hir::AssocItemContainer::Trait(_) => None,
        };

        (trait_name, trait_ref)
    }

    /// `(is_const, is_async, is_unsafe)`. These are already computed (and
    /// flattened into the `kind` string) by `ItemKindDisplayName`; here they
    /// are surfaced as structured booleans.
    fn descriptor_modifiers(&self, node: &Node) -> (bool, bool, bool) {
        match node.hir {
            hir::ModuleDef::Function(function_hir) => {
                let is_const = function_hir.is_const(self.db);
                let is_async = function_hir.is_async(self.db);
                let is_unsafe = function_hir.is_unsafe_to_call(self.db, None, self.edition);
                (is_const, is_async, is_unsafe)
            }
            hir::ModuleDef::Trait(trait_hir) => (false, false, trait_hir.is_unsafe(self.db)),
            _ => (false, false, false),
        }
    }

    fn descriptor_macro_kind(&self, node: &Node) -> Option<String> {
        let hir::ModuleDef::Macro(macro_hir) = node.hir else {
            return None;
        };

        let kind = match macro_hir.kind(self.db) {
            MacroKind::Declarative | MacroKind::DeclarativeBuiltIn => "macro_rules!",
            MacroKind::Derive | MacroKind::DeriveBuiltIn => "derive",
            MacroKind::Attr | MacroKind::AttrBuiltIn => "attr",
            MacroKind::ProcMacro => "fn-like",
        };

        Some(kind.to_owned())
    }

    /// Source position (file + 1-based line range) of the item, resolved via
    /// `analyzer::item_source_span`. `None` for source-less items.
    fn node_position(&self, node: &Node) -> Option<JsonSpan> {
        analyzer::item_source_span(node.hir, self.db, self.vfs).map(|span| JsonSpan {
            file: span.file.to_string_lossy().into_owned(),
            start_line: span.start_line,
            end_line: span.end_line,
        })
    }

    /// Rendered function signature (`fn(<params>) -> <ret>`). The rendering is
    /// produced by `rust-analyzer`'s `HirDisplay` and can therefore vary
    /// between analyzer versions — that is why it lives behind `--rich` and is
    /// not part of the default, version-stable descriptor.
    fn descriptor_signature(&self, node: &Node) -> Option<String> {
        let hir::ModuleDef::Function(function_hir) = node.hir else {
            return None;
        };

        let display_target = self.display_target();

        let mut params: Vec<String> = Vec::new();
        if function_hir.has_self_param(self.db) {
            params.push("self".to_owned());
        }
        params.extend(
            function_hir
                .params_without_self(self.db)
                .iter()
                .map(|param| param.ty().display(self.db, display_target).to_string()),
        );

        let ret = function_hir
            .ret_type(self.db)
            .display(self.db, display_target)
            .to_string();

        Some(format!("fn({}) -> {ret}", params.join(", ")))
    }

    fn descriptor_generics(&self, node: &Node) -> Option<Vec<JsonGeneric>> {
        let generic_def: hir::GenericDef = match node.hir {
            hir::ModuleDef::Function(function_hir) => function_hir.into(),
            hir::ModuleDef::Adt(adt_hir) => adt_hir.into(),
            hir::ModuleDef::Trait(trait_hir) => trait_hir.into(),
            hir::ModuleDef::TypeAlias(type_alias_hir) => type_alias_hir.into(),
            hir::ModuleDef::Const(const_hir) => const_hir.into(),
            hir::ModuleDef::Static(static_hir) => static_hir.into(),
            _ => return None,
        };

        let mut generics: Vec<JsonGeneric> = Vec::new();

        for param in generic_def.params(self.db) {
            match param {
                hir::GenericParam::TypeParam(type_param) => {
                    let name = type_param
                        .name(self.db)
                        .display(self.db, self.edition)
                        .to_string();
                    let bounds = type_param
                        .trait_bounds(self.db)
                        .iter()
                        .map(|trait_hir| {
                            trait_hir.name(self.db).display(self.db, self.edition).to_string()
                        })
                        .collect();
                    generics.push(JsonGeneric { name, bounds });
                }
                hir::GenericParam::ConstParam(const_param) => {
                    let name = const_param
                        .name(self.db)
                        .display(self.db, self.edition)
                        .to_string();
                    generics.push(JsonGeneric {
                        name,
                        bounds: Vec::new(),
                    });
                }
                // Lifetimes are intentionally excluded from the descriptor
                // (see investigation §2.5 — low value for the impact-radius
                // question at module-graph granularity).
                hir::GenericParam::LifetimeParam(_) => {}
            }
        }

        if generics.is_empty() {
            None
        } else {
            Some(generics)
        }
    }
}

fn relation_name(edge: &Edge) -> &'static str {
    edge.display_name()
}

fn uses_kind_name(edge: &Edge) -> Option<&'static str> {
    match edge {
        Relationship::Uses(kind) => Some(kind.name()),
        Relationship::Owns => None,
    }
}

fn visibility_string(visibility: &ItemVisibility) -> String {
    match visibility {
        ItemVisibility::Public => "pub".to_owned(),
        ItemVisibility::Crate => "pub(crate)".to_owned(),
        ItemVisibility::Module(path) => format!("pub(in crate::{path})"),
        ItemVisibility::Super => "pub(super)".to_owned(),
        ItemVisibility::Private => "priv".to_owned(),
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
