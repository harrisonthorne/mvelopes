use crate::{amount::Amount, errors::*};
use nom::Finish;
use std::{collections::HashSet, fmt};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClassicPosting {
    amount: Option<Amount>,
    account: String,
    cost_assertion: Option<Cost>,
    balance_assertion: Option<Amount>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EnvelopePosting {
    account_name: String,
    envelope_name: String,
    amount: Amount,
}

impl EnvelopePosting {
    pub fn new(account_name: &str, amount: Amount, envelope_name: &str) -> Self {
        Self {
            account_name: account_name.to_owned(),
            envelope_name: envelope_name.to_owned(),
            amount,
        }
    }

    /// Returns the name of the envelope associated with this posting
    pub fn get_envelope_name(&self) -> &String {
        &self.envelope_name
    }

    /// Returns the account name associated with this posting
    pub fn get_account_name(&self) -> &String {
        &self.account_name
    }

    /// Returns the amount associated with this posting
    pub fn get_amount(&self) -> &Amount {
        &self.amount
    }

    pub fn validate(&self, account_names: &HashSet<&String>) -> Result<(), ValidationError> {
        if !account_names.contains(&self.account_name) {
            let message = format!(
                "the account `{}` is not defined in your journal",
                self.account_name
            );
            Err(ValidationError::default().set_message(&message))
        } else {
            Ok(())
        }
    }
}

impl Default for EnvelopePosting {
    fn default() -> Self {
        Self {
            account_name: String::new(),
            envelope_name: String::new(),
            amount: Amount::zero(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Posting {
    Classic(ClassicPosting),
    Envelope(EnvelopePosting),
}

impl From<ClassicPosting> for Posting {
    fn from(p: ClassicPosting) -> Self {
        Self::Classic(p)
    }
}

impl From<EnvelopePosting> for Posting {
    fn from(p: EnvelopePosting) -> Self {
        Self::Envelope(p)
    }
}

impl Posting {
    #[deprecated = "the `silverfox::parsing` module provides tools for parsing silverfox data. this function uses that module internally, but scraps any leftover characters not part of the parsed amount"]
    pub fn parse(
        line: &str,
        decimal_symbol: char,
    ) -> Result<Self, ParseError> {
        crate::parsing::posting::parse_posting(decimal_symbol)(line)
            .finish()
            .map(|(_, p)| p)
    }

    /// Returns the Posting's Amount
    pub fn get_amount(&self) -> Option<&Amount> {
        match self {
            Self::Envelope(e) => Some(&e.amount),
            Self::Classic(c) => c.amount.as_ref(),
        }
    }

    /// Returns the Posting's account
    pub fn get_account(&self) -> &String {
        match self {
            Self::Classic(c) => &c.account,
            Self::Envelope(e) => &e.account_name,
        }
    }

    pub fn get_original_native_value(&self) -> Option<f64> {
        match self {
            Self::Envelope(_) => None, // not applicable to envelope postings
            Self::Classic(c) => c.get_original_native_value(),
        }
    }

    // TODO later
    // pub fn get_native_value_now(&self, prices: Prices) -> Option<f64> {
    //     match self {
    //         Self::Envelope(e) => None, // not applicable to envelope postings
    //         Self::Classic(c) => c.get_original_native_value(),
    //     }
    // }

    /// Returns a String that can be written in a file and parsed later on, giving the same result
    pub fn as_parsable(&self) -> String {
        format!("{}", self)
    }

    pub fn is_envelope(&self) -> bool {
        matches!(self, Self::Envelope(_))
    }

    pub fn is_classic(&self) -> bool {
        matches!(self, Self::Classic(_))
    }

    pub fn validate(&self, account_names: &HashSet<&String>) -> Result<(), ValidationError> {
        match self {
            Self::Classic(c) => c.validate(account_names),
            Self::Envelope(e) => e.validate(account_names),
        }
    }
}

impl fmt::Display for Posting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Classic(c) => c.fmt(f),
            Self::Envelope(e) => e.fmt(f),
        }
    }
}

impl ClassicPosting {
    pub fn new(
        account: &str,
        amount: Option<Amount>,
        cost_assertion: Option<Cost>,
        balance_assertion: Option<Amount>,
    ) -> Self {
        Self {
            account: String::from(account),
            amount,
            balance_assertion,
            cost_assertion,
        }
    }

    fn blank() -> Self {
        Self {
            amount: None,
            account: String::new(),
            cost_assertion: None,
            balance_assertion: None,
        }
    }

    fn validate(&self, accounts: &HashSet<&String>) -> Result<(), ValidationError> {
        if !accounts.contains(&self.account) {
            let message = format!(
                "the account `{}` is not defined in your journal",
                self.account
            );
            Err(ValidationError::default().set_message(message.as_str()))
        } else {
            Ok(())
        }
    }

    pub fn get_original_native_value(&self) -> Option<f64> {
        // calculate native price of this posting. posting.amount must exist for this to work
        // (since this is literally used primarily for calculating the value of blank posting
        // amounts, boi)
        if let Some(a) = &self.amount {
            if a.symbol.is_none() {
                // if the posting's amount is native, then of course that's the native amount
                Some(a.mag)
            } else {
                // otherwise, if the cost assertion is a native amount, we'll use that to
                // determine the native value
                match &self.cost_assertion {
                    Some(c) => match c {
                        Cost::TotalCost(b) => {
                            if b.symbol.is_none() {
                                Some(b.mag)
                            } else {
                                None
                            }
                        }
                        Cost::UnitCost(b) => {
                            if b.symbol.is_none() {
                                Some(a.mag * b.mag)
                            } else {
                                None
                            }
                        }
                    },
                    None => None,
                }
            }
        } else {
            None
        }
    }
}

impl fmt::Display for ClassicPosting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut postlude = String::new();

        if let Some(a) = &self.amount {
            postlude.push_str(&format!("{}", a));
        }

        if let Some(c) = &self.cost_assertion {
            postlude.push_str(&format!(" {}", c));
        }

        if let Some(b) = &self.balance_assertion {
            postlude.push_str(&format!(" ! {}", b));
        }

        write!(f, "{:50} {}", self.account, postlude)
    }
}

impl fmt::Display for EnvelopePosting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prelude = format!("envelope {} {}", self.envelope_name, self.account_name);
        write!(f, "{:50} {}", prelude, self.amount)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cost {
    TotalCost(Amount),
    UnitCost(Amount),
}

impl fmt::Display for Cost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TotalCost(a) => write!(f, " == {}", a),
            Self::UnitCost(a) => write!(f, " @ {}", a),
        }
    }
}
