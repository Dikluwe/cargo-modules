#[macro_use]
mod util;

use std::collections::HashSet;

use serde_json::Value;

use crate::util::{cmd, output};

fn run_export_json(project: &str, extra_args: &[&str]) -> Value {
    let mut args: Vec<String> = vec!["export-json".to_owned()];
    args.extend(extra_args.iter().map(|s| (*s).to_owned()));
    let mut command = cmd(project, args.iter());
    command.env("NO_COLOR", "1");
    let (stdout, stderr) = output(command, true);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("invalid JSON for project {project:?}: {err}\nSTDERR: {stderr}\nSTDOUT: {stdout}"))
}

#[test]
fn json_is_well_formed_on_minimal_crate() {
    let value = run_export_json("package_lib_target", &[]);
    let obj = value.as_object().expect("top-level object");

    let mut keys: Vec<&String> = obj.keys().collect();
    keys.sort();
    assert_eq!(keys, ["crate", "edges", "nodes"]);

    assert_eq!(obj["crate"].as_str(), Some("package_lib_target"));

    let nodes = obj["nodes"].as_array().expect("nodes is array");
    assert_eq!(nodes.len(), 1, "empty crate should only have the crate root node");

    let only = &nodes[0];
    assert_eq!(only["path"].as_str(), Some("package_lib_target"));
    assert_eq!(only["name"].as_str(), Some("package_lib_target"));
    assert_eq!(only["kind"].as_str(), Some("crate"));
    assert_eq!(only["visibility"].as_str(), Some("pub"));
    assert!(
        only["id"].is_u64(),
        "every node must carry an integer `id`; got: {only}"
    );

    let edges = obj["edges"].as_array().expect("edges is array");
    assert!(edges.is_empty(), "empty crate should have no edges");
}

#[test]
fn node_fields_are_faithful() {
    let value = run_export_json("struct_fields", &[]);
    let nodes = value["nodes"].as_array().expect("nodes is array");

    let find = |path: &str| -> &Value {
        nodes
            .iter()
            .find(|n| n["path"].as_str() == Some(path))
            .unwrap_or_else(|| panic!("missing node {path}"))
    };

    // Crate root.
    let krate = find("struct_fields");
    assert_eq!(krate["name"].as_str(), Some("struct_fields"));
    assert_eq!(krate["kind"].as_str(), Some("crate"));
    assert_eq!(krate["visibility"].as_str(), Some("pub"));

    // A `pub struct`.
    let strukt = find("struct_fields::Struct");
    assert_eq!(strukt["name"].as_str(), Some("Struct"));
    assert_eq!(strukt["kind"].as_str(), Some("struct"));
    assert_eq!(strukt["visibility"].as_str(), Some("pub"));

    // A `pub(crate)` trait (top-level item without explicit visibility).
    let trayt = find("struct_fields::TargetTrait");
    assert_eq!(trayt["name"].as_str(), Some("TargetTrait"));
    assert_eq!(trayt["kind"].as_str(), Some("trait"));
    assert_eq!(trayt["visibility"].as_str(), Some("pub(crate)"));

    // A `pub(crate)` type alias.
    let type_alias = find("struct_fields::TypeAlias");
    assert_eq!(type_alias["name"].as_str(), Some("TypeAlias"));
    assert_eq!(type_alias["kind"].as_str(), Some("type"));
    assert_eq!(type_alias["visibility"].as_str(), Some("pub(crate)"));
}

#[test]
fn both_owns_and_uses_edges_are_emitted_with_correct_direction() {
    let value = run_export_json("struct_fields", &[]);
    let edges = value["edges"].as_array().expect("edges is array");

    let has_edge = |from: &str, to: &str, relation: &str| -> bool {
        edges.iter().any(|e| {
            e["from"].as_str() == Some(from)
                && e["to"].as_str() == Some(to)
                && e["relation"].as_str() == Some(relation)
        })
    };

    // owns: crate owns its top-level item.
    assert!(
        has_edge("struct_fields", "struct_fields::Struct", "owns"),
        "expected `owns` edge from crate to struct"
    );

    // uses: Struct uses TargetStruct via a field. Direction matters.
    assert!(
        has_edge("struct_fields::Struct", "struct_fields::TargetStruct", "uses"),
        "expected `uses` edge from Struct to TargetStruct"
    );
    // The reverse direction should NOT exist for this relation.
    assert!(
        !has_edge("struct_fields::TargetStruct", "struct_fields::Struct", "uses"),
        "reverse direction must not be present"
    );

    // At least one of each relation must exist overall.
    let relations: HashSet<&str> = edges
        .iter()
        .filter_map(|e| e["relation"].as_str())
        .collect();
    assert!(relations.contains("owns"), "no `owns` edges found");
    assert!(relations.contains("uses"), "no `uses` edges found");

    // Only `owns` and `uses` are valid relations.
    for relation in &relations {
        assert!(
            *relation == "owns" || *relation == "uses",
            "unexpected relation: {relation}"
        );
    }
}

#[test]
fn uses_edges_carry_reference_vs_import_subtype() {
    // The `github_issue_102` fixture has a module `a` that both imports a type
    // (`use self::b::X;`) and contains a struct `Z` whose field references it
    // (`x: X`). That gives the two distinct `uses` subtypes between the same
    // target `a::b::X`:
    //   - module `a` -> `a::b::X` via the `use`           => "import"
    //   - struct `a::Z` -> `a::b::X` via the field type   => "reference"
    let value = run_export_json("github_issue_102", &[]);
    let edges = value["edges"].as_array().expect("edges is array");

    let uses_kind = |from: &str, to: &str| -> Option<&str> {
        edges
            .iter()
            .find(|e| {
                e["from"].as_str() == Some(from)
                    && e["to"].as_str() == Some(to)
                    && e["relation"].as_str() == Some("uses")
            })
            .and_then(|e| e["uses_kind"].as_str())
    };

    assert_eq!(
        uses_kind("github_issue_102::a", "github_issue_102::a::b::X"),
        Some("import"),
        "the module's `use` of X must be tagged as an import"
    );
    assert_eq!(
        uses_kind("github_issue_102::a::Z", "github_issue_102::a::b::X"),
        Some("reference"),
        "the struct field referencing X must be tagged as a reference"
    );

    // `owns` edges never carry a `uses_kind`, and `uses_kind` is only ever one
    // of the two known values.
    for edge in edges {
        match edge["relation"].as_str() {
            Some("owns") => assert!(
                edge.get("uses_kind").is_none(),
                "owns edge must not carry uses_kind: {edge}"
            ),
            Some("uses") => {
                let kind = edge["uses_kind"]
                    .as_str()
                    .unwrap_or_else(|| panic!("uses edge must carry uses_kind: {edge}"));
                assert!(
                    kind == "reference" || kind == "import",
                    "unexpected uses_kind: {kind}"
                );
            }
            other => panic!("unexpected relation: {other:?}"),
        }
    }
}

#[test]
fn every_edge_endpoint_references_an_existing_node() {
    // Coverage guarantee: nothing in the graph is silently dropped.
    // Each edge endpoint path must appear in the nodes list.
    let value = run_export_json("struct_fields", &[]);

    let node_paths: HashSet<&str> = value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["path"].as_str().unwrap())
        .collect();

    for edge in value["edges"].as_array().unwrap() {
        let from = edge["from"].as_str().unwrap();
        let to = edge["to"].as_str().unwrap();
        assert!(node_paths.contains(from), "edge.from {from:?} not in nodes");
        assert!(node_paths.contains(to), "edge.to {to:?} not in nodes");
    }
}

#[test]
fn every_edge_id_endpoint_references_an_existing_node_id() {
    // Per-node identity invariant: for every edge, `id_from` and `id_to`
    // resolve to a node that is present in the `nodes` array. Without this,
    // downstream consumers cannot trust the new id fields.
    let value = run_export_json("struct_fields", &[]);

    let node_ids: HashSet<u64> = value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_u64().expect("every node must have integer id"))
        .collect();

    for edge in value["edges"].as_array().unwrap() {
        let id_from = edge["id_from"]
            .as_u64()
            .expect("every edge must have integer id_from");
        let id_to = edge["id_to"]
            .as_u64()
            .expect("every edge must have integer id_to");
        assert!(
            node_ids.contains(&id_from),
            "edge.id_from {id_from} not in node ids"
        );
        assert!(
            node_ids.contains(&id_to),
            "edge.id_to {id_to} not in node ids"
        );
    }
}

#[test]
fn node_ids_are_unique_within_json() {
    // The `id` field must uniquely identify a node within one emitted JSON.
    // Two nodes sharing an id would defeat the purpose of the field.
    let value = run_export_json("struct_fields", &[]);

    let ids: Vec<u64> = value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_u64().unwrap())
        .collect();

    let unique: HashSet<u64> = ids.iter().copied().collect();
    assert_eq!(
        ids.len(),
        unique.len(),
        "node ids must be unique; got {} nodes but only {} distinct ids",
        ids.len(),
        unique.len()
    );
}

#[test]
fn colliding_paths_have_distinct_ids_and_edges() {
    // Path-collision motivation test: when two distinct nodes share the same
    // canonical `path` (this happens e.g. with an inherent + trait impl of
    // the same method), the `id` field must still tell them apart, and the
    // edges involving each must reference the correct `id_from`/`id_to`.
    let value = run_export_json("colliding_paths", &[]);

    let colliding_path = "colliding_paths::S::duplicated";
    let colliding_nodes: Vec<&Value> = value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["path"].as_str() == Some(colliding_path))
        .collect();

    assert_eq!(
        colliding_nodes.len(),
        2,
        "fixture is designed to produce two nodes with path {colliding_path:?}; \
         got {} — has the fixture or the analyzer behavior changed?",
        colliding_nodes.len()
    );

    let id_a = colliding_nodes[0]["id"].as_u64().unwrap();
    let id_b = colliding_nodes[1]["id"].as_u64().unwrap();
    assert_ne!(
        id_a, id_b,
        "colliding-path nodes must have distinct ids: both got {id_a}"
    );

    // Each colliding node must be the `id_to` of at least one `owns` edge
    // coming from the parent struct — independently. If both `owns` edges
    // pointed to the same id, we'd be hiding the collision again.
    let owns_targets: HashSet<u64> = value["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| {
            e["from"].as_str() == Some("colliding_paths::S")
                && e["to"].as_str() == Some(colliding_path)
                && e["relation"].as_str() == Some("owns")
        })
        .map(|e| e["id_to"].as_u64().unwrap())
        .collect();

    assert!(
        owns_targets.contains(&id_a) && owns_targets.contains(&id_b),
        "each colliding node must appear as id_to of an `owns` edge; got owns_targets={owns_targets:?}, ids=({id_a}, {id_b})"
    );
}

fn nodes_with_path<'a>(value: &'a Value, path: &str) -> Vec<&'a Value> {
    value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["path"].as_str() == Some(path))
        .collect()
}

fn node_with_path<'a>(value: &'a Value, path: &str) -> &'a Value {
    let matches = nodes_with_path(value, path);
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one node with path {path:?}, got {}",
        matches.len()
    );
    matches[0]
}

#[test]
fn trait_field_distinguishes_colliding_trait_methods() {
    // Two distinct traits (`Alpha`, `Beta`) impl'd for the same type, each with
    // a method named `shared`. The methods collide on `path`; the `trait` field
    // must tell them apart. (Analogue of `Display::fmt` vs `Debug::fmt`, with
    // user-defined traits so no `--sysroot` is needed.)
    let value = run_export_json("trait_descriptor", &[]);

    let colliding = nodes_with_path(&value, "trait_descriptor::S::shared");
    assert_eq!(
        colliding.len(),
        2,
        "fixture should produce two colliding `shared` nodes"
    );

    let mut traits: Vec<&str> = colliding
        .iter()
        .map(|n| {
            n["trait"]
                .as_str()
                .unwrap_or_else(|| panic!("colliding trait method must carry a `trait`: {n}"))
        })
        .collect();
    traits.sort_unstable();
    assert_eq!(
        traits,
        ["Alpha", "Beta"],
        "the two `shared` methods must have distinct trait names"
    );
}

#[test]
fn trait_ref_distinguishes_same_trait_different_args() {
    // One generic trait `Convert<T>` impl'd twice with different arguments. The
    // bare `trait` is "Convert" for both; only `trait_ref` distinguishes them.
    // (Analogue of `From<A>` vs `From<B>`.)
    let value = run_export_json("trait_descriptor", &[]);

    let colliding = nodes_with_path(&value, "trait_descriptor::S::convert");
    assert_eq!(colliding.len(), 2, "expected two colliding `convert` nodes");

    // Bare trait name coincides.
    for node in &colliding {
        assert_eq!(
            node["trait"].as_str(),
            Some("Convert"),
            "both `convert` methods belong to trait `Convert`"
        );
    }

    // trait_ref (with args) differs.
    let mut refs: Vec<&str> = colliding
        .iter()
        .map(|n| {
            n["trait_ref"]
                .as_str()
                .unwrap_or_else(|| panic!("impl method must carry a `trait_ref`: {n}"))
        })
        .collect();
    refs.sort_unstable();
    assert_eq!(
        refs,
        ["Convert<A>", "Convert<B>"],
        "trait_ref must carry the distinguishing generic arguments"
    );
}

#[test]
fn inherent_impl_method_has_no_trait() {
    let value = run_export_json("trait_descriptor", &[]);
    let inherent = node_with_path(&value, "trait_descriptor::S::inherent");
    assert!(
        inherent.get("trait").is_none() || inherent["trait"].is_null(),
        "inherent method must have no trait; got: {inherent}"
    );
    assert!(
        inherent.get("trait_ref").is_none() || inherent["trait_ref"].is_null(),
        "inherent method must have no trait_ref; got: {inherent}"
    );
}

#[test]
fn kind_modifiers_are_additive() {
    // The structured modifier booleans are added *alongside* the `kind` string,
    // which must stay exactly as before.
    let value = run_export_json("trait_descriptor", &[]);

    let const_fn = node_with_path(&value, "trait_descriptor::a_const_fn");
    assert_eq!(const_fn["is_const"].as_bool(), Some(true));
    assert_eq!(
        const_fn["kind"].as_str(),
        Some("const fn"),
        "kind string must remain unchanged"
    );

    let async_fn = node_with_path(&value, "trait_descriptor::an_async_fn");
    assert_eq!(async_fn["is_async"].as_bool(), Some(true));
    assert_eq!(async_fn["kind"].as_str(), Some("async fn"));

    let unsafe_fn = node_with_path(&value, "trait_descriptor::an_unsafe_fn");
    assert_eq!(unsafe_fn["is_unsafe"].as_bool(), Some(true));
    assert_eq!(unsafe_fn["kind"].as_str(), Some("unsafe fn"));
}

#[test]
fn non_exhaustive_is_emitted() {
    let value = run_export_json("trait_descriptor", &[]);
    let enom = node_with_path(&value, "trait_descriptor::NonExhaustiveEnum");
    assert_eq!(
        enom["is_non_exhaustive"].as_bool(),
        Some(true),
        "the `#[non_exhaustive]` enum must carry is_non_exhaustive=true"
    );
}

#[test]
fn rich_fields_are_absent_without_flag() {
    let value = run_export_json("trait_descriptor", &[]);
    for node in value["nodes"].as_array().unwrap() {
        assert!(
            node.get("signature").is_none(),
            "no `signature` without --rich; got: {node}"
        );
        assert!(
            node.get("generics").is_none(),
            "no `generics` without --rich; got: {node}"
        );
    }
}

#[test]
fn rich_fields_are_present_with_flag() {
    let value = run_export_json("trait_descriptor", &["--rich"]);

    let generic_fn = node_with_path(&value, "trait_descriptor::generic_fn");
    assert!(
        generic_fn["signature"].as_str().is_some(),
        "expected a rendered signature with --rich; got: {generic_fn}"
    );

    let generics = generic_fn["generics"]
        .as_array()
        .unwrap_or_else(|| panic!("expected generics array with --rich; got: {generic_fn}"));
    let names: Vec<&str> = generics
        .iter()
        .map(|g| g["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"T"),
        "generic parameter `T` must be listed; got: {names:?}"
    );
}

#[test]
fn preexisting_fields_are_unchanged() {
    // Backwards-compatibility guard: the original schema fields are all still
    // present on every node/edge, regardless of the new descriptor.
    let value = run_export_json("trait_descriptor", &[]);

    for node in value["nodes"].as_array().unwrap() {
        for key in ["id", "path", "name", "kind", "visibility"] {
            assert!(
                node.get(key).is_some(),
                "node missing pre-existing field {key:?}: {node}"
            );
        }
    }
    for edge in value["edges"].as_array().unwrap() {
        for key in ["from", "to", "relation", "id_from", "id_to"] {
            assert!(
                edge.get(key).is_some(),
                "edge missing pre-existing field {key:?}: {edge}"
            );
        }
    }
}

#[test]
fn compact_flag_produces_single_line() {
    let mut command = cmd("package_lib_target", ["export-json", "--compact"].iter().map(|s| s.to_string()).collect::<Vec<_>>().iter());
    command.env("NO_COLOR", "1");
    let (stdout, _stderr) = output(command, true);

    // The CLI appends a single trailing newline from `println!`.
    let trimmed = stdout.trim_end_matches('\n');
    assert!(
        !trimmed.contains('\n'),
        "compact output should be single-line, got:\n{stdout}"
    );

    // Still valid JSON.
    let value: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["crate"].as_str(), Some("package_lib_target"));
}

mod help {
    test_cmd!(
        args: "export-json --help",
        success: true,
        color_mode: ColorMode::Plain,
        project: smoke
    );
}
