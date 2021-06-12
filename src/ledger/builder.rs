use crate::{entry::builder::EntryBuilder, errors::SilverfoxResult};
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use super::Ledger;

pub struct LedgerBuilder {
    original_file_path: PathBuf,
    date_format: String,
    default_currency_symbol: Option<String>,
    decimal_symbol: char,
    accounts: Vec<AccountBuilder>,
    entry_builders: Vec<EntryBuilder>,
}

impl LedgerBuilder {
    fn new(original_file_path: impl AsRef<Path>) -> Self {
        Self {
            original_file_path: original_file_path.as_ref().to_path_buf(),
            date_format: String::from("%Y/%m/%d"),
            default_currency_symbol: None,
            decimal_symbol: '.',
            account_names: HashSet::new(),
            entry_builders: Vec::new(),
        }
    }

    fn date_format(mut self, date_format: &str) -> Self {
        self.date_format = date_format.to_string();
        self
    }

    fn default_currency_symbol(mut self, symbol: &str) -> Self {
        self.default_currency_symbol = Some(symbol.to_string());
        self
    }

    fn decimal_symbol(mut self, symbol: char) -> Self {
        self.decimal_symbol = symbol;
        self
    }

    fn account_name(mut self, name: &str) -> Self {
        self.account_names.insert(name.to_string());
        self
    }

    fn entry(mut self, builder: EntryBuilder) -> Self {
        // NOTE: should this function call `build()` on the `builder`? that way, entries can be
        // validated and silverfox can terminate asap in case of a validation problem
        self.entry_builders.push(builder);
        self
    }

    fn build(self) -> SilverfoxResult<Ledger> {
        let entries = {
            let v = Vec::new();
            for b in self.entry_builders {
                v.push(b.build()?);
            }
            v
        };

        Ok(Ledger {
            original_file_path: self.original_file_path,
            entries,
            date_format: self.date_format,
            accounts: (),
            default_currency_symbol: self.default_currency_symbol,
            decimal_symbol: self.decimal_symbol,
        })
    }
}
