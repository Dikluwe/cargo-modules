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
