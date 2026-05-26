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
//!   "nodes": [ { "path": "...", "name": "...", "kind": "...", "visibility": "..." } ],
//!   "edges": [ { "from": "...", "to": "...", "relation": "owns" | "uses" } ]
//! }
//! ```

use hir::db::HirDatabase;
use ra_ap_hir::{self as hir};
use ra_ap_ide::Edition;

use petgraph::visit::{IntoNodeReferences, NodeRef};
use serde::Serialize;

use crate::{
    analyzer,
    graph::{Edge, Graph, Node},
    item::ItemVisibility,
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
    path: String,
    name: String,
    kind: String,
    visibility: String,
}

#[derive(Serialize)]
struct JsonEdge {
    from: String,
    to: String,
    relation: &'static str,
}

pub struct Printer<'a> {
    options: &'a Options,
    krate: hir::Crate,
    db: &'a dyn HirDatabase,
    edition: Edition,
}

impl<'a> Printer<'a> {
    pub fn new(
        options: &'a Options,
        krate: hir::Crate,
        db: &'a dyn HirDatabase,
        edition: Edition,
    ) -> Self {
        Self {
            options,
            krate,
            db,
            edition,
        }
    }

    pub fn to_json(&self, graph: &Graph<Node, Edge>) -> anyhow::Result<String> {
        let krate_name = analyzer::crate_name(self.krate, self.db);

        let mut nodes: Vec<JsonNode> = graph
            .node_references()
            .map(|node_ref| self.node_to_json(node_ref.weight()))
            .collect();
        nodes.sort_by(|a, b| a.path.cmp(&b.path));

        let mut edges: Vec<JsonEdge> = graph
            .edge_indices()
            .map(|edge_idx| {
                let (source_idx, target_idx) = graph.edge_endpoints(edge_idx).unwrap();
                let edge = &graph[edge_idx];
                JsonEdge {
                    from: graph[source_idx].display_path(self.db, self.edition),
                    to: graph[target_idx].display_path(self.db, self.edition),
                    relation: relation_name(edge),
                }
            })
            .collect();
        edges.sort_by(|a, b| {
            a.from
                .cmp(&b.from)
                .then_with(|| a.to.cmp(&b.to))
                .then_with(|| a.relation.cmp(b.relation))
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

    fn node_to_json(&self, node: &Node) -> JsonNode {
        let path = node.display_path(self.db, self.edition);
        let name = node.display_name(self.db, self.edition);
        let kind = node.kind_display_name(self.db, self.edition).to_string();
        let visibility = visibility_string(&node.visibility(self.db, self.edition));

        JsonNode {
            path,
            name,
            kind,
            visibility,
        }
    }
}

fn relation_name(edge: &Edge) -> &'static str {
    edge.display_name()
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
