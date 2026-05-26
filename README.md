# cargo-modules

[![Downloads](https://img.shields.io/crates/d/cargo-modules.svg?style=flat-square)](https://crates.io/crates/cargo-modules/)
[![Version](https://img.shields.io/crates/v/cargo-modules.svg?style=flat-square)](https://crates.io/crates/cargo-modules/)
[![License](https://img.shields.io/crates/l/cargo-modules.svg?style=flat-square)](https://crates.io/crates/cargo-modules/)

## Synopsis

A cargo plugin for visualizing/analyzing a crate's internal structure.

## Motivation

With time, as your Rust projects grow bigger and bigger, it gets more and more important to properly structure your code.
Fortunately Rust provides us with a quite sophisticated module system, allowing us to neatly split up our crates into arbitrarily small sub-modules of types and functions.
While this helps to avoid monolithic and unstructured chunks of code, it can also make it hard at times to still mentally stay on top of the over-all high-level structure of the project at hand.

This is where `cargo-modules` comes into play:

## Installation

Install `cargo-modules` via:

```bash
cargo install cargo-modules
```

## Usage

The `cargo-modules` tool comes with a couple of commands:

```bash
# Print a crate's hierarchical structure as a tree:
cargo modules structure <OPTIONS>

# Print a crate's internal dependencies as a graph:
cargo modules dependencies <OPTIONS>

# Detect unlinked source files within a crate's directory:
cargo modules orphans <OPTIONS>

# Export a crate's internal dependency graph as structured JSON:
cargo modules export-json <OPTIONS>
```

<details>
<summary>Command help</summary>

```terminal
$ cargo modules --help

Visualize/analyze a crate's internal structure.

Usage: cargo-modules <COMMAND>

Commands:
  structure     Prints a crate's hierarchical structure as a tree.
  dependencies  Prints a crate's internal dependencies as a graph.
  orphans       Detects unlinked source files within a crate's directory.
  export-json   Exports a crate's internal dependency graph as structured JSON.
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

</details>

### cargo modules structure

Print a crate's hierarchical structure as a tree:

```bash
cargo modules structure <OPTIONS>
```

<details>
<summary>Command help</summary>

```terminal
$ cargo modules structure --help

Prints a crate's hierarchical structure as a tree.

Usage: cargo-modules structure [OPTIONS]

Options:
      --verbose                        Use verbose output
      --lib                            Process only this package's library
      --bin <BIN>                      Process only the specified binary
  -p, --package <PACKAGE>              Package to process (see `cargo help pkgid`)
      --no-default-features            Do not activate the `default` feature
      --all-features                   Activate all available features
      --features <FEATURES>            List of features to activate. This will be ignored if `--cargo-all-features` is provided
      --target <TARGET>                Analyze for target triple
      --manifest-path <MANIFEST_PATH>  Path to Cargo.toml [default: .]
      --no-fns                         Filter out functions (e.g. fns, async fns, const fns) from tree
      --no-traits                      Filter out traits (e.g. trait, unsafe trait) from tree
      --no-types                       Filter out types (e.g. structs, unions, enums) from tree
      --sort-by <SORT_BY>              The sorting order to use (e.g. name, visibility, kind) [default: name]
      --sort-reversed                  Reverses the sorting order
      --focus-on <FOCUS_ON>            Focus the graph on a particular path or use-tree's environment, e.g. "foo::bar::{self, baz, blee::*}"
      --max-depth <MAX_DEPTH>          The maximum depth of the generated graph relative to the crate's root node, or nodes selected by '--focus-on'
      --cfg-test                       Analyze with `#[cfg(test)]` enabled (i.e as if built via `cargo test`)
  -h, --help                           Print help
```

</details>

#### Example: Modules Structure as Text Tree

```bash
cd ./tests/projects/readme_tree_example
cargo-modules structure --cfg-test
```

Output:

![Output of `cargo modules structure …`](docs/structure_output.png)

```rust
crate readme_tree_example
├── trait Lorem: pub
├── mod amet: pub(crate)
│   └── mod consectetur: pub(self)
│       └── mod adipiscing: pub(self)
│           └── union Elit: pub(in crate::amet)
├── mod dolor: pub(crate)
│   └── enum Sit: pub(crate)
└── mod tests: pub(crate) #[cfg(test)]
    └── fn it_works: pub(self) #[test]
```

(Project source code: [readme_tree_example/src/lib.rs](./tests/projects/readme_tree_example/src/lib.rs))

#### Terminal Colors

If you are running the command on a terminal with color support and don't have `NO_COLOR` defined in your environment, then the output will be colored for easier visual parsing:

```plain
└── <visibility> <keyword> <name> [<test-attributes>]
```

The `<visibility>` ([more info](https://doc.rust-lang.org/reference/visibility-and-privacy.html)) is furthermore highlighted by the following colors:

| Color    | Meaning                                                                            |
| -------- | ---------------------------------------------------------------------------------- |
| 🟢 green  | Items visible to all and everything (i.e. `pub`)                                   |
| 🟡 yellow | Items visible to the current crate (i.e. `pub(crate)`)                             |
| 🟠 orange | Items visible to a certain parent module (i.e. `pub(in path)`)                     |
| 🔴 red    | Items visible to the current module (i.e. `pub(self)`, implied by lack of `pub …`) |

The `<keyword>` is highlighted in 🔵 blue to visually separate it from the name.

Test-guarded items (i.e. `#[cfg(test)] …`) and test functions (i.e. `#[test] fn …`) have their corresponding `<test-attributes>` printed next to them in gray and cyan.

### cargo modules dependencies

Print a crate's internal dependencies as a graph:

```bash
cargo modules dependencies <OPTIONS>
```

<details>
<summary>Command help</summary>

```terminal
$ cargo modules dependencies --help

Prints a crate's internal dependencies as a graph.

Usage: cargo-modules dependencies [OPTIONS]

Options:
      --verbose                        Use verbose output
      --lib                            Process only this package's library
      --bin <BIN>                      Process only the specified binary
  -p, --package <PACKAGE>              Package to process (see `cargo help pkgid`)
      --no-default-features            Do not activate the `default` feature
      --all-features                   Activate all available features
      --features <FEATURES>            List of features to activate. This will be ignored if `--cargo-all-features` is provided
      --target <TARGET>                Analyze for target triple
      --manifest-path <MANIFEST_PATH>  Path to Cargo.toml [default: .]
      --no-externs                     Filter out extern items from extern crates from graph
      --no-fns                         Filter out functions (e.g. fns, async fns, const fns) from graph
      --no-modules                     Filter out modules (e.g. `mod foo`, `mod foo {}`) from graph
      --no-sysroot                     Filter out sysroot crates (`std`, `core` & friends) from graph
      --no-traits                      Filter out traits (e.g. trait, unsafe trait) from graph
      --no-types                       Filter out types (e.g. structs, unions, enums) from graph
      --no-uses                        Filter out "use" edges from graph
      --acyclic                        Require graph to be acyclic
      --layout <LAYOUT>                The graph layout algorithm to use (e.g. none, dot, neato, twopi, circo, fdp, sfdp) [default: neato]
      --focus-on <FOCUS_ON>            Focus the graph on a particular path or use-tree's environment, e.g. "foo::bar::{self, baz, blee::*}"
      --max-depth <MAX_DEPTH>          The maximum depth of the generated graph relative to the crate's root node, or nodes selected by '--focus-on'
      --cfg-test                       Analyze with `#[cfg(test)]` enabled (i.e as if built via `cargo test`)
  -h, --help                           Print help


        If you have xdot installed on your system, you can run this using:
        `cargo modules dependencies | xdot -`
```

</details>

#### Example: Graphical Module Structure

```bash
cargo modules dependencies --no-externs --no-fns --no-sysroot --no-traits --no-types --no-uses > mods.dot
```

(The command above is equivalent to `cargo-modules generate graph` from v0.12.0 or earlier.)

![Output of `cargo modules dependencies …`](docs/dependencies_mods_only_output.svg)

#### Example: Graphical Dependencies

```bash
cd ./tests/projects/smoke
cargo-modules dependencies --cfg-test | dot -Tsvg
```

![Output of `cargo modules dependencies …`](docs/dependencies_output.svg)

```plain
See "./docs/dependencies_output.dot" for the corresponding raw dot file.
```

(Project source code: [readme_graph_example/src/lib.rs](./tests/projects/readme_graph_example/src/lib.rs))

#### Node Structure

The individual nodes are structured as follows:

```plain
┌────────────────────────┐
│ <visibility> <keyword> │
├────────────────────────┤
│         <path>         │
└────────────────────────┘
```

#### Node Colors

The `<visibility>` ([more info](https://doc.rust-lang.org/reference/visibility-and-privacy.html)) is furthermore highlighted by the following colors:

| Color    | Meaning                                                                            |
| -------- | ---------------------------------------------------------------------------------- |
| 🔵 blue   | Crates (i.e. their implicit root module)                                           |
| 🟢 green  | Items visible to all and everything (i.e. `pub`)                                   |
| 🟡 yellow | Items visible to the current crate (i.e. `pub(crate)`)                             |
| 🟠 orange | Items visible to a certain parent module (i.e. `pub(in path)`)                     |
| 🔴 red    | Items visible to the current module (i.e. `pub(self)`, implied by lack of `pub …`) |

#### Acyclic Mode

cargo-modules's `dependencies` command checks for the presence of a `--acyclic` flag. If found it will search for cycles in the directed graph and return an error for any cycles it found.

Running `cargo modules dependencies --lib --acyclic` on the source of the tool itself emits the following cycle error:

```plain
Error: Circular dependency between `cargo_modules::options::general` and `cargo_modules::options::generate`.

┌> cargo_modules::options::general
│  └─> cargo_modules::options::generate::graph
│      └─> cargo_modules::options::generate
└──────────┘
```

### cargo modules orphans

Detect unlinked source files within a crate's directory:

```bash
cargo modules orphans <OPTIONS>
```

<details>
<summary>Command help</summary>

```terminal
$ cargo modules orphans --help

Detects unlinked source files within a crate's directory.

Usage: cargo-modules orphans [OPTIONS]

Options:
      --verbose                        Use verbose output
      --lib                            Process only this package's library
      --bin <BIN>                      Process only the specified binary
  -p, --package <PACKAGE>              Package to process (see `cargo help pkgid`)
      --no-default-features            Do not activate the `default` feature
      --all-features                   Activate all available features
      --features <FEATURES>            List of features to activate. This will be ignored if `--cargo-all-features` is provided
      --target <TARGET>                Analyze for target triple
      --manifest-path <MANIFEST_PATH>  Path to Cargo.toml [default: .]
      --deny                           Returns a failure code if one or more orphans are found
      --cfg-test                       Analyze with `#[cfg(test)]` enabled (i.e as if built via `cargo test`)
  -h, --help                           Print help
```

</details>

#### Example

```bash
cd ./tests/projects/orphans
cargo-modules orphans
```

Output:

![Output of `cargo modules structure …`](docs/orphans_output.png)

```plain
2 orphans found:

warning: orphaned module `foo` at src/orphans/foo/mod.rs
  --> src/orphans.rs
   |  ^^^^^^^^^^^^^^ orphan module not loaded from file
   |
 help: consider loading `foo` from module `orphans::orphans`
   |
   |  mod foo;
   |  ++++++++
   |

warning: orphaned module `bar` at src/orphans/bar.rs
  --> src/orphans.rs
   |  ^^^^^^^^^^^^^^ orphan module not loaded from file
   |
 help: consider loading `bar` from module `orphans::orphans`
   |
   |  mod bar;
   |  ++++++++
   |

Error: Found 2 orphans in crate 'orphans'
```

(Project source code: [orphans/src/lib.rs](./tests/projects/orphans/src/lib.rs))

### cargo modules export-json

Export a crate's internal dependency graph as structured JSON, for consumption by other tools (linters, dashboards, downstream analyzers).

Canonical invocation:

```bash
cargo modules export-json --sysroot --compact
```

<details>
<summary>Command help</summary>

```terminal
$ cargo modules export-json --help

Exports a crate's internal dependency graph as structured JSON.

Usage: cargo-modules export-json [OPTIONS]

Options:
      --verbose                        Use verbose output
      --lib                            Process only this package's library
      --bin <BIN>                      Process only the specified binary
  -p, --package <PACKAGE>              Package to process (see `cargo help pkgid`)
      --no-default-features            Do not activate the `default` feature
      --all-features                   Activate all available features
      --features <FEATURES>            List of features to activate. This will be ignored if `--cargo-all-features` is provided
      --target <TARGET>                Analyze for target triple
      --manifest-path <MANIFEST_PATH>  Path to Cargo.toml [default: .]
      --cfg-test                       Analyze with `#[cfg(test)]` enabled (i.e as if built via `cargo test`)
      --compact                        Emit JSON on a single line (no indentation)
      --sysroot                        Include sysroot crates (`std`, `core` & friends) in the exported graph
  -h, --help                           Print help
```

</details>

#### Flags

- `--sysroot` — loads `std`/`core`/`alloc` in the analyzer's database. This is **required for fidelity**: without it, items synthesized by `#[derive(...)]` (e.g. the `Clone::clone` method generated by `#[derive(Clone)]`) are not expanded and the corresponding `uses` edges are missing from the graph. With it, the graph reflects the real reachability — at the cost of some stdlib nodes appearing in the output.
- `--compact` — emits the JSON on a single line (no pretty-print). Useful for piping to `jq -c` or other line-based consumers.
- `--cfg-test` — analyzes with `#[cfg(test)]` enabled, like the other subcommands.
- Target selection (`--lib`, `--bin <BIN>`, `-p <PACKAGE>`, `--manifest-path`, `--features`, `--target`, etc.) — identical to the other subcommands.

#### Output shape

```json
{
  "crate": "<crate name>",
  "nodes": [
    { "path": "...", "name": "...", "kind": "...", "visibility": "..." }
  ],
  "edges": [
    { "from": "...", "to": "...", "relation": "owns" | "uses" }
  ]
}
```

Nodes are sorted by `path`; edges by `(from, to, relation)`. The output is deterministic and stable across runs.

| Field | Meaning |
|-------|---------|
| `nodes[].path` | Canonical path (e.g. `my_crate::module::Item`). Acts as the node's unique identity. |
| `nodes[].name` | Short item name. |
| `nodes[].kind` | One of: `crate`, `mod`, `fn`, `const fn`, `async fn`, `unsafe fn`, `struct`, `union`, `enum`, `variant`, `const`, `static`, `trait`, `unsafe trait`, `type`, `builtin`, `macro`. |
| `nodes[].visibility` | One of: `pub`, `pub(crate)`, `pub(in crate::<path>)`, `pub(super)`, `priv`. |
| `edges[].from` / `edges[].to` | Canonical `path` of the source/target node. Direction is preserved. |
| `edges[].relation` | `owns` (containment: a module contains an item) or `uses` (a use: one item refers to another). |

#### Note: `--sysroot` is opt-in by design

The flag is **off by default**. The fork is a neutral tool that exposes the capability; whether to invoke it with `--sysroot` is a policy decision that lives in the consuming project, not here.

### No-Color Mode

cargo-modules checks for the presence of a `NO_COLOR` environment variable that, when present (regardless of its value), prevents the addition of color to the console output (and only the console output!).

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our [code of conduct](https://www.rust-lang.org/conduct.html),  
and the process for submitting pull requests to us.

## Versioning

We use [SemVer](http://semver.org/) for versioning. For the versions available, see the [tags on this repository](https://github.com/regexident/cargo-modules/tags).

## License

This project is licensed under the [**MPL-2.0**](https://www.tldrlegal.com/l/mpl-2.0) – see the [LICENSE.md](LICENSE.md) file for details.
