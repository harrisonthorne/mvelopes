use nom::Finish;

use crate::amount::{Amount, AmountPool};
use crate::errors::*;
use crate::posting::Posting;
use crate::utils;
use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

pub mod builder;

#[derive(Debug, Eq, PartialEq)]
pub enum EntryStatus {
    /// `?`
    Pending,
    /// `~`
    Cleared,
    /// `*`
    Reconciled,
}

impl EntryStatus {
    pub fn from_char(c: char) -> Result<Self, ParseError> {
        Self::from_str(&format!("{}", c))
    }

    pub fn to_char(&self) -> char {
        match self {
            EntryStatus::Pending => '?',
            EntryStatus::Cleared => '~',
            EntryStatus::Reconciled => '*',
        }
    }
}

impl FromStr for EntryStatus {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "?" => Ok(EntryStatus::Pending),
            "~" => Ok(EntryStatus::Cleared),
            "*" => Ok(EntryStatus::Reconciled),
            _ => Err(ParseError {
                message: Some(format!("silverfox requires statuses on entries and `{}` is not a status that silverfox understands", s)),
                context: None,
            })
        }
    }
}

impl fmt::Display for EntryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_char())
    }
}

pub struct Entry {
    date: chrono::NaiveDate,
    status: EntryStatus,
    description: String,
    payee: Option<String>,
    comment: Option<String>,

    /// The postings in this Entry. This cannot be changed because Accounts and Envelopes process
    /// entries only once. Any modifications to entries can't be reflected elsewhere on the fly.
    postings: Vec<Posting>,
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Entry {{ date: {}, status: {}, description: {}, payee: {:?}, comment: {:?}, postings: {:?} }}",
            self.date, self.status, self.description, self.payee, self.comment, self.postings
        )
    }
}

impl Entry {
    #[deprecated = "the `silverfox::parsing` module provides tools for parsing silverfox data. this function uses that module internally, but scraps any leftover characters not part of the parsed entry"]
    pub fn parse(
        chunk: &str,
        date_format: &str,
        decimal_symbol: char,
        account_names: &HashSet<&String>,
    ) -> Result<Self, SilverfoxError> {
        let (_, entry) = crate::parsing::entry::parse_entry(date_format, decimal_symbol)(chunk).finish()?;
        entry.build(account_names)
    }

    pub fn get_blank_amount(&self) -> Result<Option<Amount>, ProcessingError> {
        if !self.has_blank_posting() {
            // return None if the Entry has no blank amount
            Ok(None)
        } else {
            // calculation of the blank amount depends on whether or not multiple currencies exist
            if self.has_mixed_currencies() {
                // if multiple currencies exist, attempt to return the sum of the native amounts.
                // if any of the native amounts are None, the calculation fails and this function
                // returns an error
                let mut blank_amount = Amount::zero();
                for posting in &self.postings {
                    match posting.get_original_native_value() {
                        Some(v) => blank_amount.mag -= v,
                        None => {
                            // native_value will be None for the blank amount, so only throw an
                            // error if the posting's amount is Some
                            if posting.get_amount().is_some() {
                                let err = ProcessingError::default().set_message(
                                    "silverfox couldn't infer a value for an entry's blank posting amount. there are
multiple currencies in this entry, but one posting does not provide its
currency's worth in your native currency.").set_context(&self.as_full_string());
                                return Err(err);
                            }
                        }
                    }
                }

                Ok(Some(blank_amount))
            } else {
                // for each posting, subtract that posting's amount from the blank amount (as long as
                // `posting` doesn't have a blank amount)
                let mut iter = self.postings.iter();

                // get starting blank amount by finding the first non-blank amount and then
                // negating it
                let mut blank_amount = if let Some(p) = iter.find(|p| p.get_amount().is_some()) {
                    if let Some(a) = p.get_amount() {
                        -a.clone()
                    } else {
                        // shouldn't be reachable, since preceding closure asserts this amount is
                        // not blank
                        unreachable!()
                    }
                } else {
                    // shouldn't be reachable, since there'd better be at least one non-blank
                    // amount
                    unreachable!()
                };

                // subtract the rest of the postings
                for posting in iter {
                    if let Some(a) = posting.get_amount() {
                        blank_amount -= a.clone();
                    }
                }

                Ok(Some(blank_amount))
            }
        }
    }

    /// Checks that the Entry is valid. Returns a ValidationError if it is invalid. An Entry is
    /// valid when all of the following are true:
    ///
    /// - it contains no more than one blank posting amount
    /// - it's balanced (the sum of its postings equals zero)
    /// - it contains no more than one type of currency when a blank posting amount exists (later
    ///   to be supported)
    fn validate(&self, context: &str) -> Result<(), ValidationError> {
        let mut blank_amounts = 0;
        let mut symbol_set = HashSet::new();
        for posting in &self.postings {
            // does amount exist?
            if let Some(a) = posting.get_amount() {
                // if so, add its symbol to the set if it exists
                if let Some(s) = &a.symbol {
                    symbol_set.insert(s);
                }
            } else {
                blank_amounts += 1;

                // if more than one blank amount, quit here and throw an error
                if blank_amounts > 1 {
                    return Err(ValidationError::default()
                        .set_message("a single entry can't have more than one blank posting")
                        .set_context(context));
                }
            }
        }

        // if there's a blank amount but the currencies aren't consistent, we can't infer the
        // blank's amount; there's a way around this that will be worked out in the future, but for
        // now it will be unsupported: TODO
        if blank_amounts > 0 && symbol_set.len() > 1 {
            return Err(ValidationError::default().set_message("silverfox can't infer the amount of a blank posting when other postings have mixed currencies").set_context(context));
        }

        Ok(())
    }

    pub fn as_full_string(&self) -> String {
        let payee = if let Some(p) = &self.payee {
            p
        } else {
            "No payee"
        };

        let mut s = format!(
            "{} {} {} [{}]",
            self.date, self.status, self.description, payee
        );
        for posting in &self.postings {
            s.push_str(&format!("\n\t{}", posting));
        }

        s
    }

    pub fn get_envelope_postings(&self) -> Vec<Posting> {
        let mut clone = self.postings.clone();
        clone.retain(|p| p.is_envelope());
        clone
    }

    pub fn get_date(&self) -> &chrono::NaiveDate {
        &self.date
    }

    pub fn contains_account_posting(&self, account_name: &str) -> bool {
        self.postings
            .iter()
            .any(|p| p.get_account() == account_name)
    }

    pub fn get_postings(&self) -> &Vec<Posting> {
        &self.postings
    }

    pub fn has_blank_posting(&self) -> bool {
        for posting in &self.postings {
            if posting.get_amount().is_none() {
                return true;
            }
        }

        false
    }

    pub fn has_envelope_posting(&self) -> bool {
        for posting in &self.postings {
            if posting.is_envelope() {
                return true;
            }
        }

        false
    }

    pub fn has_mixed_currencies(&self) -> bool {
        if self.postings.is_empty() {
            false
        } else {
            let mut iter = self.postings.iter();
            let symbol_to_match = if let Some(p) = iter.find(|&p| p.get_amount().is_some()) {
                if let Some(a) = p.get_amount() {
                    a.symbol.clone()
                } else {
                    unreachable!()
                }
            } else {
                None
            };

            for posting in iter {
                // this code was copied and pasted from above, maybe consider writing a function
                let posting_symbol = match posting.get_amount() {
                    Some(posting_amount) => posting_amount.symbol.clone(),
                    None => continue,
                };

                if posting_symbol != symbol_to_match {
                    return true;
                }
            }

            false
        }
    }

    pub fn as_register_data(
        &self,
        date_format: &str,
        account_match: &Option<String>,
    ) -> Result<Option<EntryRegisterData>, ProcessingError> {
        // XXX: This closure is a duplicate of the one in
        // `ledger::display_register()`
        let is_account_name_focused = |account_name: &str| match account_match {
            Some(match_str) => account_name.contains(match_str),
            // TODO: an issue ticket is open to further solidify whether or not an account is an
            // "asset", so this will be changed soon (it's kinda dumb right now)
            None => account_name.starts_with("asset"),
        };

        let (positive_name, negative_name, amounts) = {
            let mut positive_names = HashSet::new();
            let mut negative_names = HashSet::new();
            let mut focused_amount = AmountPool::new();

            for p in &self.postings {
                let name = p.get_account();
                let amount = if let Some(a) = p.get_amount() {
                    a.clone()
                } else {
                    self.get_blank_amount()?.unwrap()
                };

                if amount.mag > 0.0 {
                    positive_names.insert(name);
                } else if amount.mag < 0.0 {
                    negative_names.insert(name);
                }

                if is_account_name_focused(name) {
                    focused_amount += amount;
                }
            }

            if focused_amount.is_zero() {
                // if there are no focused accounts in this entry, we won't
                // worry about this entry's output
                return Ok(None);
            }

            let positive_name = match positive_names.len() {
                0 => "(none)".to_string(),
                1 => positive_names.iter().next().unwrap().to_string(),
                _ => "(multiple)".to_string(),
            };

            let negative_name = match negative_names.len() {
                0 => "(none)".to_string(),
                1 => negative_names.iter().next().unwrap().to_string(),
                _ => "(multiple)".to_string(),
            };

            (positive_name, negative_name, focused_amount)
        };

        let account_flow = (negative_name.clone(), positive_name.clone());
        let short_account_flow = (
            negative_name.split(':').last().unwrap().to_string(),
            positive_name.split(':').last().unwrap().to_string(),
        );
        let single_account_display = {
            if !is_account_name_focused(&positive_name) {
                positive_name.split(':').last().unwrap()
            } else if !is_account_name_focused(&negative_name) {
                negative_name.split(':').last().unwrap()
            } else {
                // both positive and negative accounts are focused, so this is
                // probably a conversion
                "(conversion)"
            }
        }
        .to_string();

        Ok(Some(EntryRegisterData {
            date: self.date.format(date_format).to_string(),
            status: self.status.to_char(),
            description: self.description.clone(),
            payee: self
                .payee
                .as_ref()
                .map(|p| format!("[{}]", p))
                .unwrap_or_else(|| "".to_string()),
            account_flow,
            short_account_flow,
            single_account_display,
            amounts,
        }))
    }

    pub fn as_parsable(&self, date_format: &str) -> String {
        let date = self.date.format(date_format);

        let mut s = String::new();

        match &self.payee {
            Some(p) => match &self.comment {
                Some(c) => {
                    s.push_str(
                        format!(
                            "{} {} {} [{}] // {}\n",
                            date, self.status, self.description, p, c
                        )
                        .as_str(),
                    );
                }
                None => {
                    s.push_str(
                        format!("{} {} {} [{}]\n", date, self.status, self.description, p).as_str(),
                    );
                }
            },
            None => match &self.comment {
                Some(c) => {
                    s.push_str(
                        format!("{} {} {} // {}\n", date, self.status, self.description, c)
                            .as_str(),
                    );
                }
                None => {
                    s.push_str(format!("{} {} {}\n", date, self.status, self.description).as_str());
                }
            },
        }

        for posting in &self.postings {
            s.push_str(format!("    {}\n", posting.as_parsable()).as_str());
        }

        s
    }
}

pub struct EntryRegisterData {
    pub date: String,
    pub status: char,
    pub description: String,
    pub payee: String,
    pub account_flow: (String, String),       // from, to
    pub short_account_flow: (String, String), // from, to
    pub single_account_display: String,
    pub amounts: AmountPool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY_STR: &str = "2019/08/02 * Groceries [Grocery store]
            assets:checking    -50
            expenses:groceries  50";

    #[test]
    fn parse_test() {
        let mut accounts: HashSet<&String> = HashSet::new();
        let checking_name = String::from("assets:checking");
        let expenses_name = String::from("expenses:groceries");
        accounts.insert(&checking_name);
        accounts.insert(&expenses_name);

        match Entry::parse(ENTRY_STR, "%Y/%m/%d", '.', &accounts) {
            Ok(e) => {
                assert_eq!(
                    e.date,
                    chrono::NaiveDate::from_ymd(2019, 8, 2),
                    "date was not parsed correctly"
                );
                assert_eq!(
                    e.status,
                    EntryStatus::Reconciled,
                    "status was not parsed correctly"
                );
                assert_eq!(
                    e.description,
                    String::from("Groceries"),
                    "description was not parsed correctly"
                );
                assert_eq!(
                    e.payee,
                    Some(String::from("Grocery store")),
                    "payee was not parsed correctly"
                );
                assert_eq!(e.postings.len(), 2, "postings should have two items");
            }
            Err(e) => panic!("{}", e),
        };
    }
}
