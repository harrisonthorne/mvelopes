#[macro_use]
extern crate nom;

pub mod account;
pub mod amount;
pub mod entry;
pub mod envelope;
pub mod errors;
pub mod flags;
pub mod importer;
pub mod ledger;
pub mod parsing;
pub mod posting;
pub mod utils;

fn main() {
    match flags::CommandFlags::parse_from_env() {
        Ok(f) => if let Err(e) = f.execute() {
            eprintln!("{}", e)
        },
        Err(e) => eprintln!("{}", e),
    }
}
