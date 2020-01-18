use crate::account::Account;
use crate::amount::{Amount, AmountPool};
use crate::entry::Entry;
use crate::errors::*;
use crate::utils;
use crate::posting::Posting;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::fs;

pub struct Ledger {
    file_path: PathBuf,
    entries: Vec<Entry>,
    date_format: String, // default = "%Y/%m/%d"
    accounts: HashMap<String, Account>,
    default_currency: String,
    decimal_symbol: char,
}

impl Ledger {
    /// Returns a blank ledger, with default values for `date_format` and `decimal_symbol`
    fn blank() -> Self {
        Ledger {
            file_path: PathBuf::new(),
            date_format: String::from("%Y/%m/%d"),
            entries: Vec::<Entry>::new(),
            accounts: HashMap::new(),
            default_currency: String::new(),
            decimal_symbol: '.',
        }
    }

    /// Given a `file_path`, returns an entire file's contents as a String
    fn get_string_from_file(file_path: &Path) -> Result<String, MvelopesError> {
        let path_display = file_path.display();
        let mut file = match File::open(file_path) {
            Ok(f) => f,
            Err(e) => return Err(MvelopesError::from(BasicError {
                message: format!("couldn't open `{}`: {}", path_display, e)
            }))
        };

        let mut s = String::new();
        if let Err(e) = file.read_to_string(&mut s) {
            return Err(MvelopesError::from(BasicError {
                message: format!("couldn't read `{}`: {}", path_display, e)
            }))
        }

        Ok(s)
    }

    /// Returns a ledger parsed from a file at the `file_path`
    pub fn from_file(file_path: &Path) -> Result<Self, MvelopesError> {
        let mut ledger = Self::blank();
        ledger.file_path = PathBuf::from(file_path);

        if let Err(e) = ledger.add_from_file(file_path) {
            Err(e)
        } else {
            Ok(ledger)
        }
    }

    /// Adds to the ledger from the contents parsed from the file at the `file_path`
    fn add_from_file(&mut self, file_path: &Path) -> Result<(), MvelopesError> {
        let s = match Self::get_string_from_file(file_path) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        if let Some(parent) = file_path.parent() {
            self.add_from_str(&s, parent)
        } else {
            Err(MvelopesError::from(ParseError::default().set_message("a file without a valid parent can't be used")))
        }
    }

    /// Adds to the ledger from the contents parsed from the string
    fn add_from_str(&mut self, s: &str, parent_path: &Path) -> Result<(), MvelopesError> {
        // init a chunk
        let mut chunk = String::new();

        // split lines
        let lines = s.lines();
        for mut line in lines {
            line = utils::remove_comments(line.trim_end());

            // if the first character of this line is whitespace, it is part of the current chunk.
            // if there is no first character, nothing happens
            if let Some(c) = line.chars().next() {
                if c.is_whitespace() {
                    chunk.push('\n');
                    chunk.push_str(line);
                } else {
                    if let Err(e) = self.parse_chunk(&chunk, parent_path) {
                        return Err(e);
                    }
                    chunk = String::from(line);
                }
            }
        }

        // parse the last chunk
        if let Err(e) = self.parse_chunk(&chunk, parent_path) {
            Err(e)
        } else {
            Ok(())
        }
    }

    /// Parses a single chunk and adds its contents to the ledger. Returns an MvelopesError is
    /// there was an issue in validation or in parsing.
    ///
    /// What is a "chunk"? A "chunk" starts at a line that starts with a non-whitespace character
    /// and ends before the next line that starts with a non-whitespace character.
    fn parse_chunk(&mut self, chunk: &str, parent_path: &Path) -> Result<(), MvelopesError> {
        if chunk.is_empty() {
            return Ok(()); // blank chunks are fine; they don't modify anything, so no error needed
        }

        let mut tokens = chunk.split_whitespace();
        let keyword = tokens.next();
        let value = tokens.next();
        match keyword {
            None => Ok(()),
            Some("account") => self.parse_account(chunk),
            Some("currency") => self.set_currency(value),
            Some("date_format") => self.set_date_format(value),
            Some("include") => self.include(value, parent_path),
            _ => self.parse_entry(chunk),
        }
    }

    /// Parses a currency symbol
    fn set_currency(&mut self, cur: Option<&str>) -> Result<(), MvelopesError> {
        match cur {
            None => Err(MvelopesError::from(ParseError {
                message: Some("no currency provided, but currency keyword was found".to_string()),
                context: None,
            })),
            Some(c) => {
                self.default_currency = c.into();
                Ok(())
            }
        }
    }

    fn set_date_format(&mut self, date_format: Option<&str>) -> Result<(), MvelopesError> {
        match date_format {
            None => Err(MvelopesError::from(ParseError {
                context: None,
                message: Some(
                    "no date format provided, but date_format keyword was found".to_string(),
                ),
            })),
            Some(d) => {
                self.date_format = d.into();
                Ok(())
            }
        }
    }

    fn include(&mut self, file: Option<&str>, parent_path: &Path) -> Result<(), MvelopesError> {
        match file {
            None => Err(MvelopesError::from(
                ParseError::default().set_message("no file provided to an `include` clause"),
            )),
            Some(f) => self.add_from_file(&parent_path.join(f)),
        }
    }

    fn parse_entry(&mut self, chunk: &str) -> Result<(), MvelopesError> {
        match Entry::parse(
            chunk,
            &self.date_format,
            self.decimal_symbol,
            &self.accounts.keys().collect(),
        ) {
            Ok(entry) => self.add_entry(entry),
            Err(e) => Err(e),
        }
    }

    fn add_entry(&mut self, entry: Entry) -> Result<(), MvelopesError> {
        for (_, account) in self.accounts.iter_mut() {
            if let Err(e) = account.process_entry(&entry) {
                return Err(MvelopesError::from(e));
            }
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Appends the entry to the file of the Ledger, then internally adds the Entry itself to the
    /// Ledger.
    fn append_entry(&mut self, entry: Entry) -> Result<(), MvelopesError> {
        let mut file = match fs::File::with_options().append(true).open(&self.file_path) {
            Ok(f) => {
                f
            },
            Err(e) => {
                return Err(MvelopesError::from(e))
            }
        };

        if let Err(e) = write!(file, "\n{}", entry.as_parsable(&self.date_format)) {
            return Err(MvelopesError::from(BasicError {
                message: format!("{}", e)
            }))
        }

        self.add_entry(entry)
    }

    fn parse_account(&mut self, chunk: &str) -> Result<(), MvelopesError> {
        match Account::parse(chunk, self.decimal_symbol, &self.date_format) {
            Ok(a) => {
                self.accounts.insert(a.get_name().to_string(), a);
                Ok(())
            },
            Err(e) => Err(e),
        }
    }

    pub fn display_flat_balance(&self) -> Result<(), MvelopesError> {
        let totals_map = match self.get_totals() {
            Ok(m) => m,
            Err(e) => return Err(e),
        };

        let mut totals_vec = totals_map.iter().collect::<Vec<(&String, &AmountPool)>>();
        totals_vec.sort_by(|a, b| a.0.cmp(b.0));

        for pair in totals_vec.iter() {
            println!("{:35}    {}", pair.0, pair.1);
        }

        Ok(())
    }

    // TODO This can be rewritten, since totals are accounted for within the Account struct
    fn get_totals(&self) -> Result<HashMap<String, AmountPool>, MvelopesError> {
        // map for account names to amount pools
        let mut totals_map: HashMap<String, AmountPool> = HashMap::new();

        // read: for each posting in the ledger, add its amount to its account in totals_map
        for entry in &self.entries {
            for posting in entry.get_postings() {
                let posting_amount = posting.get_amount();
                let posting_account = posting.get_account();
                // if the account key exists, just add to it. if it doesn't exist, insert a new key
                // with the amount
                match totals_map.get_mut(posting_account) {
                    Some(pool) => {
                        if let Some(a) = posting_amount {
                            *pool += a.clone();
                        } else {
                            match entry.get_blank_amount() {
                                Ok(o) => {
                                    if let Some(b) = o {
                                        *pool += b;
                                    }
                                }
                                Err(e) => return Err(MvelopesError::from(e)),
                            }
                        }
                    }
                    None => {
                        // if the posting amount exists, set an AmountPool from the amount as the
                        // key's value. otherwise, use an AmountPool from a zero Amount.
                        if let Some(a) = posting_amount {
                            totals_map
                                .insert(posting_account.to_owned(), AmountPool::from(a.clone()));
                        } else {
                            totals_map.insert(
                                posting_account.to_owned(),
                                AmountPool::from(Amount::zero()),
                            );
                        }
                    }
                }
            }
        }

        Ok(totals_map)
    }

    pub fn display_envelopes(&self) {
        let mut account_keys = self.accounts.keys().collect::<Vec<&String>>();
        account_keys.sort();
        for key in account_keys {
            let account = &self.accounts[key];
            account.display_envelopes();
        }
    }

    pub fn fill_envelopes(&mut self) -> Result<(), MvelopesError> {
        let mut postings: Vec<Posting> = Vec::new();
        for account in self.accounts.values() {
            postings.append(&mut account.get_filling_postings())
        }

        // remove zero-magnitude postings, they're useless
        postings.retain(|p| if let Some(a) = p.get_amount() {
            a.mag != 0.0
        } else {
            false
        });

        // if no postings exist, forget adding an entry
        if postings.is_empty() {
            return Ok(())
        }

        let entry = Entry::new(
            chrono::Local::today().naive_utc(),
            crate::entry::EntryStatus::Cleared,
            String::from("automatic move to envelopes (generated by mvelopes)"),
            None,
            postings,
            None
        );

        self.append_entry(entry)
    }
}

impl Debug for Ledger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for ent in &self.entries {
            ent.fmt(f)?;
            writeln!(f)?;
        }

        Ok(())
    }
}
