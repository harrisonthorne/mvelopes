use crate::amount::Amount;
use crate::errors::*;
use crate::utils;
use std::collections::HashSet;
use std::fmt;

#[derive(Clone, Debug)]
pub struct ClassicPosting {
    amount: Option<Amount>,
    account: String,
    cost_assertion: Option<Cost>,
    balance_assertion: Option<Amount>,
}

#[derive(Clone, Debug)]
pub struct EnvelopePosting {
    account_name: String,
    envelope_name: String,
    amount: Amount,
}

impl EnvelopePosting {
    pub fn new(account_name: String, amount: Amount, envelope_name: String) -> Self {
        Self {
            account_name,
            envelope_name,
            amount,
        }
    }

    pub fn parse(
        line: &str,
        decimal_symbol: char,
        _accounts: &HashSet<&String>,
    ) -> Result<Self, ParseError> {
        let mut tokens = line.split_whitespace().skip(1);

        let account_name = if let Some(a) = tokens.next() {
            String::from(a)
        } else {
            return Err(ParseError {
                message: Some("probably missing an account name".to_string()),
                context: Some(line.to_string()),
            });
        };

        let envelope_name = if let Some(e) = tokens.next() {
            String::from(e)
        } else {
            return Err(ParseError {
                message: Some("probably missing an envelope name".to_string()),
                context: Some(line.to_string()),
            });
        };

        // hopefully collects the remainder of the tokens, and not all of the beginning ones too
        let amount_tokens: String = tokens.collect();
        let amount = Amount::parse(&amount_tokens, decimal_symbol)?;

        Ok(Self {
            account_name,
            envelope_name,
            amount,
        })
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

#[derive(Clone, Debug)]
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
    pub fn parse(
        mut line: &str,
        decimal_symbol: char,
        accounts: &HashSet<&String>,
    ) -> Result<Self, SilverfoxError> {
        // match first token, to decide on parsing an envelope posting or a classic posting
        line = utils::remove_comments(line).trim();
        match line.split_whitespace().next() {
            Some(t) => {
                if t == "envelope" {
                    Ok(Posting::from(EnvelopePosting::parse(
                        line,
                        decimal_symbol,
                        accounts,
                    )?))
                } else {
                    Ok(Posting::from(ClassicPosting::parse(
                        line,
                        decimal_symbol,
                        accounts,
                    )?))
                }
            }
            None => Err(SilverfoxError::from(ParseError {
                message: Some("nothing to parse for a Posting".to_string()),
                context: None,
            })),
        }
    }

    // getters

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

    pub fn parse(
        line: &str,
        decimal_symbol: char,
        accounts: &HashSet<&String>,
    ) -> Result<Self, SilverfoxError> {
        let mut posting = Self::blank();

        // remove comments and other impurities
        let trimmed_line = utils::remove_comments(line).trim();
        let tokens = trimmed_line.split_whitespace().collect::<Vec<&str>>();

        let amount_tokens: Vec<&str>;
        posting.account = tokens[0].to_string();
        amount_tokens = tokens[1..].to_vec();

        if let Err(e) = posting.parse_amount(&amount_tokens, decimal_symbol) {
            Err(SilverfoxError::from(e))
        } else if let Err(e) = posting.parse_assertion_amounts(&amount_tokens, decimal_symbol) {
            Err(SilverfoxError::from(e))
        } else if let Err(e) = posting.validate(&accounts) {
            Err(SilverfoxError::from(e))
        } else {
            Ok(posting)
        }
    }

    fn parse_amount(
        &mut self,
        amount_tokens: &[&str],
        decimal_symbol: char,
    ) -> Result<(), ParseError> {
        let mut iter = amount_tokens.iter();
        let raw_amount = match iter.position(|&s| s == "@" || s == "!" || s == "=" || s == "!!") {
            Some(cutoff) => amount_tokens[..cutoff].join(" "),
            None => amount_tokens.join(" "),
        };

        if raw_amount.trim().is_empty() {
            self.amount = None;
            return Ok(());
        }

        self.amount = match Amount::parse(&raw_amount, decimal_symbol) {
            Ok(a) => Some(a),
            Err(e) => return Err(e),
        };

        Ok(())
    }

    fn parse_assertion_amounts(
        &mut self,
        amount_tokens: &[&str],
        decimal_symbol: char,
    ) -> Result<(), ParseError> {
        self.balance_assertion =
            match Self::parse_balance_assertion_amount(amount_tokens, decimal_symbol) {
                Ok(a) => a,
                Err(e) => return Err(e),
            };

        self.cost_assertion = match Self::parse_price_amount(amount_tokens, decimal_symbol) {
            Ok(price_opt) => {
                // parsing succeeded, if there is a price, use that
                if let Some(price) = price_opt {
                    Some(price)
                } else {
                    // if no price, try to parse total cost
                    match Self::parse_total_cost_amount(amount_tokens, decimal_symbol) {
                        Ok(total_cost_opt) => {
                            total_cost_opt
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            Err(e) => return Err(e),
        };

        Ok(())
    }

    fn parse_balance_assertion_amount(
        amount_tokens: &[&str],
        decimal_symbol: char,
    ) -> Result<Option<Amount>, ParseError> {
        extract_amount(amount_tokens, decimal_symbol, "!", |&s| {
            s == "!!" || s == "@" || s == "="
        })
    }

    fn parse_price_amount(
        amount_tokens: &[&str],
        decimal_symbol: char,
    ) -> Result<Option<Cost>, ParseError> {
        match extract_amount(amount_tokens, decimal_symbol, "@", |&s| {
            s == "!" || s == "!!" || s == "="
        })? {
            Some(a) => Ok(Some(Cost::UnitCost(a))),
            None => Ok(None),
        }
    }

    fn parse_total_cost_amount(
        amount_tokens: &[&str],
        decimal_symbol: char,
    ) -> Result<Option<Cost>, ParseError> {
        match extract_amount(amount_tokens, decimal_symbol, "=", |&s| {
            s == "!" || s == "!!" || s == "@"
        })? {
            Some(a) => Ok(Some(Cost::TotalCost(a))),
            None => Ok(None),
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
        let prelude = format!("envelope {} {}", self.account_name, self.envelope_name);
        write!(f, "{:50} {}", prelude, self.amount)
    }
}

#[derive(Clone, Debug)]
pub enum Cost {
    TotalCost(Amount),
    UnitCost(Amount),
}

impl fmt::Display for Cost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TotalCost(a) => write!(f, " = {}", a),
            Self::UnitCost(a) => write!(f, " @ {}", a),
        }
    }
}

fn extract_amount<P>(
    amount_tokens: &[&str],
    decimal_symbol: char,
    wanted_operator: &str,
    unwanted_op_predicate: P,
) -> Result<Option<Amount>, ParseError>
where
    P: FnMut(&&str) -> bool,
{
    // find the balance_assertion token
    let mut iter = amount_tokens.iter();
    match iter.position(|&s| s == wanted_operator) {
        Some(i) => {
            // trim unwanted tokens
            let mut useful_tokens = &amount_tokens[i + 1..];

            // find any other tokens that should be filtered out
            if let Some(i) = useful_tokens.iter().position(unwanted_op_predicate) {
                // trim unwanted tokens
                useful_tokens = &useful_tokens[..i];
            }

            // parse the amount
            match Amount::parse(useful_tokens.join(" ").as_str(), decimal_symbol) {
                Ok(a) => Ok(Some(a)),
                Err(e) => Err(e),
            }
        }
        None => Ok(None),
    }
}
