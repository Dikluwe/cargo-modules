// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use hir::db::HirDatabase;
use ra_ap_hir::{self as hir};
use ra_ap_ide::{self as ide};
use ra_ap_vfs::{self as vfs};

use clap::Parser;

use crate::{analyzer::LoadOptions, graph::GraphBuilder};

use super::{options::Options, printer::Printer};

#[derive(Parser, Clone, PartialEq, Eq, Debug)]
pub struct Command {
    #[command(flatten)]
    pub options: Options,
}

impl Command {
    pub fn new(options: Options) -> Self {
        Self { options }
    }

    pub(crate) fn sanitize(&mut self) {}

    #[doc(hidden)]
    pub fn run(
        self,
        krate: hir::Crate,
        db: &ide::RootDatabase,
        vfs: &vfs::Vfs,
        edition: ide::Edition,
    ) -> anyhow::Result<()> {
        let hir_db: &dyn HirDatabase = db;

        tracing::trace!("Building graph ...");

        let builder = GraphBuilder::new(hir_db, edition, krate);
        let (graph, _crate_node_idx) = builder.build()?;

        tracing::trace!("Serializing graph as JSON ...");

        // The printer holds the concrete `RootDatabase` (not just `&dyn
        // HirDatabase`): resolving source positions needs `LineIndexDatabase`,
        // which `RootDatabase` provides.
        let printer = Printer::new(&self.options, krate, db, vfs, edition);
        let json = printer.to_json(&graph)?;

        println!("{json}");

        Ok(())
    }

    pub fn load_options(&self) -> LoadOptions {
        LoadOptions {
            cfg_test: self.options.cfg_test,
            sysroot: self.options.sysroot,
        }
    }
}
