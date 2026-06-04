// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::fmt;

use petgraph::stable_graph::StableGraph;

use crate::item::Item;

mod builder;
mod walker;

pub(crate) use self::{builder::GraphBuilder, walker::GraphWalker};

pub type Graph<N, E> = StableGraph<N, E>;

pub type Node = Item;
pub type Edge = Relationship;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum Relationship {
    Uses(UsesKind),
    Owns,
}

/// Subtype of a `uses` edge, recording *how* the edge was created.
///
/// The builder fuses two semantically distinct edges under `uses`:
/// a genuine type dependency from a signature/field (`walk_and_push_type`)
/// and an `use`-declaration attributed to a module (the scope loop of
/// `process_module`). Carrying the subtype keeps both `uses` (so `relation`
/// and the other subcommands are unchanged) while making them distinguishable
/// downstream.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum UsesKind {
    /// Direct use of a type in a signature, field or return position.
    Reference,
    /// An `use` declaration attributed to the module (a scope import).
    Import,
}

impl UsesKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Import => "import",
        }
    }
}

impl Relationship {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Uses(_) => "uses",
            Self::Owns => "owns",
        }
    }
}

impl fmt::Display for Relationship {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Uses(_) => "Uses",
            Self::Owns => "Owns",
        };
        write!(f, "{name}")
    }
}
