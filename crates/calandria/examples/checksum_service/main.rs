//! Runs one bounded checksum owner singularly and across explicit static shards.

mod run;
mod service;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    run::checksum_service()
}
