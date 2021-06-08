use std::collections::HashSet;

use chrono::NaiveDate;

use crate::amount::Amount;
use crate::errors::SilverfoxResult;

use super::Envelope;
use super::{EnvelopeType, Frequency, FundingMethod};

pub struct EnvelopeBuilder {
    parent_account: String,
    envelope_type: EnvelopeType,
    name: String,

    freq: Frequency,
    starting_date: Option<chrono::NaiveDate>,

    amount: Amount,
    auto_accounts: HashSet<String>,
    funding: FundingMethod,
}

impl EnvelopeBuilder {
    pub fn new(name: &str, envelope_type: EnvelopeType, parent_account: &str) -> Self {
        Self {
            parent_account: parent_account.to_string(),
            envelope_type,
            name: name.to_string(),

            freq: Frequency::Never,
            starting_date: None,

            amount: Amount::zero(),
            auto_accounts: HashSet::new(),
            funding: FundingMethod::Manual,
        }
    }

    pub fn freq(mut self, freq: Frequency) -> Self {
        self.freq = freq;
        self
    }

    pub fn starting_date(mut self, starting_date: NaiveDate) -> Self {
        self.starting_date = Some(starting_date);
        self
    }

    pub fn amount(mut self, amount: Amount) -> Self {
        self.amount = amount;
        self
    }

    pub fn auto_account(mut self, account: &str) -> Self {
        self.auto_accounts.insert(account.to_string());
        self
    }

    pub fn funding(mut self, method: FundingMethod) -> Self {
        self.funding = method;
        self
    }

    pub fn build(self) -> SilverfoxResult<Envelope> {
        Ok(Envelope {
            name: self.name,
            amount: self.amount,
            envelope_type: self.envelope_type,
            auto_accounts: self.auto_accounts,
            freq: self.freq,
            funding: self.funding,
            starting_date: self.starting_date,
            next_amount: Amount::zero(),
            now_amount: Amount::zero(),
            parent_account: self.parent_account,
            last_transaction_date: NaiveDate::from_ymd(1, 1, 1),
        })
    }
}
