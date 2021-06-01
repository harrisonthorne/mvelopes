use crate::{
    amount::AmountPool,
    entry::Entry,
    envelope::{Envelope, EnvelopeType},
    errors::*,
    posting::Posting,
    utils,
};
use std::cmp::Ordering;

pub struct Account {
    name: String,

    // TODO: combine these into one
    expense_envelopes: Vec<Envelope>,
    goal_envelopes: Vec<Envelope>,

    /// The real, actual value of this account, which ignores envelopes or virtual postings.
    /// TODO: use this for balance statements
    real_value: AmountPool,
}

impl Account {
    pub fn parse(
        chunk: &str,
        decimal_symbol: char,
        date_format: &str,
    ) -> Result<Self, SilverfoxError> {
        let mut lines = chunk.lines();
        let header = match lines.next() {
            Some(l) => l,
            None => {
                return Err(SilverfoxError::from(ParseError {
                    context: Some(chunk.to_string()),
                    message: Some(
                        "account header can't be parsed because it doesn't exist".to_string(),
                    ),
                }))
            }
        };

        let account_name = Account::parse_header(&header.to_string())?;
        let expense_envelopes = Vec::new();
        let goal_envelopes = Vec::new();

        let mut account = Account {
            name: account_name,
            expense_envelopes,
            goal_envelopes,
            real_value: AmountPool::new(),
        };

        let mut envelope_chunk = String::new();
        for line in lines {
            let trimmed_line = line.trim();
            if trimmed_line.starts_with("expense") || trimmed_line.starts_with("goal") {
                // add a new envelope, if the chunk isn't blank
                if !envelope_chunk.trim().is_empty() {
                    let new_envelope = Envelope::parse(
                        &envelope_chunk,
                        &account.name,
                        decimal_symbol,
                        &date_format,
                    )?;

                    if let Err(e) = account.add_envelope(new_envelope) {
                        return Err(SilverfoxError::from(e));
                    }
                }

                envelope_chunk = String::from(line);
            } else {
                envelope_chunk.push('\n');
                envelope_chunk.push_str(line);
            }
        }

        // parse the remainder
        if !envelope_chunk.trim().is_empty() {
            let new_envelope =
                Envelope::parse(&envelope_chunk, &account.name, decimal_symbol, &date_format)?;

            if let Err(e) = account.add_envelope(new_envelope) {
                return Err(SilverfoxError::from(e));
            }
        }

        // finish by sorting envelopes

        Ok(account)
    }

    // returns the name of the account
    fn parse_header(mut line: &str) -> Result<String, ParseError> {
        // remove comments
        line = utils::remove_comments(line);

        let tokens = line.trim().split_whitespace().collect::<Vec<&str>>();
        match tokens.len().cmp(&2) {
            Ordering::Greater => Err(ParseError {
                context: Some(line.to_string()),
                message: Some(
                    "accounts can't have spaces in them; use underscores instead: _".to_string(),
                ),
            }),
            Ordering::Less => Err(ParseError {
                context: Some(line.to_string()),
                message: Some("blank account definition".to_string()),
            }),
            Ordering::Equal => Ok(tokens[1].to_string()),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn add_envelope(&mut self, envelope: Envelope) -> Result<(), ValidationError> {
        let envelope_collection = match envelope.get_type() {
            EnvelopeType::Expense => &mut self.expense_envelopes,
            EnvelopeType::Goal => &mut self.goal_envelopes,
        };

        let envelope_exists = envelope_collection
            .iter()
            .any(|e| e.get_name() == envelope.get_name());
        if envelope_exists {
            Err(ValidationError {
                message: Some(format!(
                    "there's a duplicate envelope definition for `{}` in the account `{}`",
                    envelope.get_name(),
                    self.name
                )),
                context: None,
            })
        } else {
            (*envelope_collection).push(envelope);
            Ok(())
        }
    }

    /// Processes the Entry by looking for any changes to envelope amounts and applying them. Also
    /// adds to the real_value of the Account.
    pub fn process_entry(&mut self, entry: &Entry) -> Result<(), ProcessingError> {
        for envelope in self
            .expense_envelopes
            .iter_mut()
            .chain(self.goal_envelopes.iter_mut())
        {
            envelope.process_entry(entry)?;
        }

        for posting in entry.get_postings() {
            if *posting.get_account() == self.name && !posting.is_envelope() {
                if let Some(a) = posting.get_amount() {
                    self.real_value += a;
                } else {
                    match entry.get_blank_amount() {
                        Ok(o) => {
                            if let Some(a) = o {
                                self.real_value += a;
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        Ok(())
    }

    pub fn display_envelopes(&self) {
        // if no envelopes to display, quit
        if self.expense_envelopes.is_empty() && self.goal_envelopes.is_empty() {
            return;
        }

        // displays account name at top
        println!("{}", self.name);

        // display available balance
        println!("  available");
        let available_value = self.get_available_value();
        for amount in available_value.iter() {
            if amount.mag == 0.0 {
                continue;
            }
            println!("    {}", amount)
        }

        // display expenses
        if !self.expense_envelopes.is_empty() {
            println!("  expenses");
            for envelope in self.expense_envelopes.iter() {
                println!("{}", envelope);
            }
        }

        // display goals
        if !self.goal_envelopes.is_empty() {
            println!("  goals");
            for envelope in self.goal_envelopes.iter() {
                println!("{}", envelope);
            }
        }

        println!(); // do not remove; this is a separator
    }

    pub fn get_filling_postings(&self) -> Vec<Posting> {
        let mut postings: Vec<Posting> = Vec::new();
        let mut available_value = self.get_available_value();

        // create an iterator, then reverse it so that envelopes are drained more safely in case
        // the account's available value is negative. goals will be drained first, starting at the
        // envelope with the farthest due date
        let iter = self
            .expense_envelopes
            .iter()
            .chain(self.goal_envelopes.iter());
        for envelope in iter.rev() {
            let new_posting = Posting::from(envelope.get_filling_posting(&available_value));
            if let Some(new_amount) = new_posting.get_amount() {
                available_value -= new_amount;
                postings.push(new_posting);
            }
        }

        postings
    }

    pub fn get_available_value(&self) -> AmountPool {
        let mut amount_pool = self.real_value.clone();
        for envelope in self
            .expense_envelopes
            .iter()
            .chain(self.goal_envelopes.iter())
        {
            amount_pool = amount_pool - envelope.get_next_amount() - envelope.get_now_amount();
        }

        amount_pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Frequency;

    const ACCOUNT_STR: &str = "account assets:checking
             goal yearly_goal due every year starting 2020/2/20
                 amount 1000 CAD
             expense groceries due every 5th
                 amount 300 USD
                 for expenses:food:groceries
                 funding conservative";

    const BLANK_ACCOUNT_STR: &str = "account ";

    const ACCOUNT_WITH_SPACES_STR: &str = "account assets bank checking";

    const DEFAULT_DATE_FORMAT: &str = "%Y/%m/%d";

    #[test]
    fn parse_test() {
        // do the thing
        let account = match Account::parse(ACCOUNT_STR, '.', DEFAULT_DATE_FORMAT) {
            Ok(a) => a,
            Err(e) => panic!("{}", e),
        };

        // test name
        assert_eq!(account.name, "assets:checking");

        // test envelopes
        {
            // expenses
            assert_eq!(
                account.expense_envelopes.len(),
                1,
                "no expense envelopes; there should be one"
            );
            let ex_envelope = &account.expense_envelopes[0];
            assert_eq!(ex_envelope.get_name(), "groceries");
            assert_eq!(*ex_envelope.get_freq(), Frequency::Monthly(5));

            // goals
            assert_eq!(
                account.goal_envelopes.len(),
                1,
                "no goal envelopes; there should be one"
            );
            let goal_envelope = &account.goal_envelopes[0];
            assert_eq!(goal_envelope.get_name(), "yearly_goal");
            assert_eq!(
                *goal_envelope.get_freq(),
                Frequency::Annually(chrono::NaiveDate::from_ymd(2020, 2, 20))
            );
        }
    }

    #[test]
    fn blank_account_test() {
        let result = Account::parse(BLANK_ACCOUNT_STR, '.', DEFAULT_DATE_FORMAT);
        assert!(result.is_err());
    }

    #[test]
    fn parse_with_spaces_test() {
        let result = Account::parse(ACCOUNT_WITH_SPACES_STR, '.', DEFAULT_DATE_FORMAT);
        assert!(result.is_err());
    }
}
