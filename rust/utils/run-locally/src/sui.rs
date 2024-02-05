use macro_rules_attribute::apply;
use crate::{logging::log, program::Program, utils::as_task};

use tempfile::{tempdir, NamedTempFile};

#[apply(as_task)]
pub fn start_sui_local_testnet() {
    log!("Runnint local Sui testnet");

    Program::new("sui console")
    .run()

}