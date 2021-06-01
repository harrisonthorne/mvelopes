use crate::amount::{Amount, AmountPool};
use crate::entry::Entry;
use crate::errors::ParseError;
use crate::errors::ProcessingError;
use crate::posting::{EnvelopePosting, Posting};
use crate::utils;
use chrono::prelude::*;
use chrono::{Local, NaiveDate};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

#[derive(Debug)]
pub struct Envelope {
    name: String,
    amount: Amount,
    envelope_type: EnvelopeType,
    auto_accounts: HashSet<String>,
    freq: Frequency,
    funding: FundingMethod,
    starting_date: Option<chrono::NaiveDate>,

    /// The amount saved up for the next due date.
    next_amount: Amount,

    /// The amount saved up now.
    now_amount: Amount,

    /// The account this Envelope pertains to
    parent_account: String,

    /// The last date at which this envelope was affected
    last_transaction_date: NaiveDate,
}

impl Ord for Envelope {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_due_date = if let Some(d) = self.get_next_due_date() {
            d
        } else if other.get_next_due_date().is_some() {
            return Ordering::Greater;
        } else {
            return Ordering::Equal;
        };

        let other_due_date = if let Some(d) = other.get_next_due_date() {
            d
        } else {
            return Ordering::Less;
        };

        self_due_date.cmp(&other_due_date)
    }
}

impl PartialOrd for Envelope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Envelope {
    fn eq(&self, other: &Self) -> bool {
        self.get_next_due_date() == other.get_next_due_date()
    }
}

impl Eq for Envelope {}

#[derive(Debug)]
pub enum EnvelopeType {
    Expense,
    Goal,
}

impl EnvelopeType {
    fn from_str(raw: &str) -> Result<Self, ParseError> {
        match raw {
            "expense" => Ok(EnvelopeType::Expense),
            "goal" => Ok(EnvelopeType::Goal),
            _ => Err(ParseError {
                context: Some(raw.to_string()),
                message: Some(
                    "this envelope type doesn't exist; instead use either `expense` or `goal`"
                        .to_string(),
                ),
            }),
        }
    }
}

#[derive(Debug)]
pub enum FundingMethod {
    Manual,
    Conservative,
    Aggressive,
}

impl FundingMethod {
    fn from_str(raw: &str) -> Result<Self, ParseError> {
        match raw.trim() {
            "manual" => Ok(FundingMethod::Manual),
            "aggressive" => Ok(FundingMethod::Aggressive),
            "conservative" => Ok(FundingMethod::Conservative),
            _ => Err(ParseError {
                context: Some(raw.to_string()),
                message: Some("this funding method doesn't exist".to_string()),
            }),
        }
    }
}

// tuples including a date is the "starting" date
#[derive(Debug, PartialEq)]
pub enum Frequency {
    Never,
    Once(NaiveDate),
    Weekly(chrono::Weekday),
    Biweekly(NaiveDate),
    Monthly(u32),
    Bimonthly(NaiveDate),
    // Quarterly(NaiveDate),
    // Semiannually(NaiveDate),
    Annually(NaiveDate),
}

impl Frequency {
    pub fn parse(
        s: &str,
        date_format: &str,
        starting_date: Option<NaiveDate>,
    ) -> Result<Self, ParseError> {
        if let Some(what) = s.strip_prefix("every other ") {
            // stop if "starting" isn't given, since it's required here
            if starting_date.is_none() {
                return Err(ParseError {
                    context: Some(s.to_string()),
                    message: Some("a `starting` clause is required for `every other` frequencies so silverfox knows which weeks or months to use".to_string())
                });
            }

            // parse "every others"
            // remember: the `starting` clause is already trimmed
            if Self::parse_weekday(what).is_some() {
                Ok(Self::Biweekly(starting_date.unwrap()))
            } else if Self::parse_day_of_month(what).is_some() {
                Ok(Self::Bimonthly(starting_date.unwrap()))
            } else {
                Err(ParseError {
                    context: Some(s.to_string()),
                    message: Some("invalid frequency".to_string()),
                })
            }
        } else if let Some(what) = s.strip_prefix("every ") {
            // parse "everys"
            // remember: the `starting` clause is already trimmed
            if let Some(w) = Self::parse_weekday(what) {
                Ok(Self::Weekly(w))
            } else if let Some(d) = Self::parse_day_of_month(what) {
                Ok(Self::Monthly(d))
            } else if what == "year" {
                match starting_date {
                    Some(d) => Ok(Self::Annually(d)),
                    None => Err(ParseError{
                        context: Some(s.to_string()),
                        message: Some("envelopes due annually require a `starting` date so that silverfox knows which day of the year the envelope is due".to_string()),
                    })
                }
            } else {
                Err(ParseError {
                    context: Some(s.to_string()),
                    message: Some("invalid frequency".to_string()),
                })
            }
        } else {
            // probably not repeating, so let's just try to parse a date
            match NaiveDate::parse_from_str(s, date_format) {
                Ok(d) => Ok(Self::Once(d)),
                Err(_) => {
                    let message = format!("couldn't parse `{}` with format `{}`", s, date_format);

                    Err(ParseError {
                        message: Some(message),
                        context: None,
                    })
                }
            }
        }
    }

    /// Parses, you know, a Weekday. Returns an Option because it may or may not exist.
    fn parse_weekday(s: &str) -> Option<chrono::Weekday> {
        match chrono::Weekday::from_str(s) {
            Ok(w) => Some(w),
            _ => None,
        }
    }

    /// Parses a day of the month. Returns an Option because it may or may not exist. Any
    /// non-digits are removed from the string to parse a number. So, technically, you could write
    /// "1stjalsdkxbuz" and it would still return 1. "2faxcbya7uw" would return 27.
    fn parse_day_of_month(s: &str) -> Option<u32> {
        // filter out any letters, spaces, etc, and parse the number in the string
        let num = s.chars().filter(|c| c.is_digit(10)).collect::<String>();

        match num.parse::<u32>() {
            Ok(n) => Some(n),
            _ => None,
        }
    }

    /// Gets the Frequency's last due date based on the next due date
    pub fn get_last_due_date(&self) -> Option<NaiveDate> {
        // get the next due date and just subtract
        match self.get_next_due_date() {
            Some(next_date) => match self {
                Self::Weekly(_) => Some(next_date - chrono::Duration::days(7)),
                Self::Biweekly(_) => Some(next_date - chrono::Duration::days(14)),
                Self::Monthly(_) => Some(Self::subtract_months(next_date, 1)),
                Self::Bimonthly(_) => Some(Self::subtract_months(next_date, 2)),
                Self::Annually(d) => Some(d.with_year(d.year() - 1).unwrap()),
                _ => None,
            },
            None => match self {
                Self::Once(d) => Some(*d),
                Self::Never => None,
                _ => unreachable!(),
            },
        }
    }

    fn subtract_months(date: NaiveDate, num: i32) -> NaiveDate {
        let mut new_month0 = date.month0() as i32 - num;
        let mut new_year = date.year();

        // this is dumb and pretty inefficient, so we'll have to improve this later. it's just the
        // easy thing to do for now. TODO
        while new_month0 < 0 {
            new_month0 += 12;
            new_year -= 1;
        }

        NaiveDate::from_ymd(new_year, new_month0 as u32 + 1, date.day())
    }

    // this function is pretty long, so we should probably break it into smaller functions
    /// Calculates and returns the next due date based on this Frequency.
    pub fn get_next_due_date(&self) -> Option<NaiveDate> {
        let today = Local::today().naive_local();
        match self {
            Self::Never => None,
            Self::Once(date) => {
                if date <= &today {
                    None
                } else {
                    Some(*date)
                }
            }
            Self::Weekly(w) => {
                // get next by weekday; keep adding to this 'next' variable until the weekday
                // matches
                let mut next = today + chrono::Duration::days(1);
                while next.weekday() != *w {
                    next += chrono::Duration::days(1);
                }

                Some(next)
            }
            Self::Biweekly(starting_date) => {
                // ATTENTION: `w` is not needed here because `starting_date` is required to be on
                // the same weekday as `w` itself

                // if starting date is after today, use that
                let duration_passed = today.signed_duration_since(*starting_date);
                let periods_passed = duration_passed.num_weeks() / 2;
                let next = *starting_date + chrono::Duration::weeks((periods_passed + 1) * 2);
                Some(next)
            }
            Self::Monthly(day_of_month) => {
                Some(Self::next_date_by_day_of_month(today, *day_of_month))
            }
            Self::Bimonthly(starting_date) => {
                if starting_date > &today {
                    Some(*starting_date)
                } else {
                    // brute force method until we find something better to do...
                    let day_of_month = starting_date.day();
                    let mut date = *starting_date;
                    while date < today {
                        let month0_plus_two = date.month0() + 2;
                        let new_year = date.year() + month0_plus_two as i32 / 12;
                        let new_month = (month0_plus_two % 12) + 1; // + 1 so it's one-based

                        // basically create a new date with the month, year and day
                        date = match NaiveDate::from_ymd_opt(new_year, new_month, day_of_month) {
                            Some(x) => x,
                            None => Self::get_last_date_of_month(NaiveDate::from_ymd(
                                new_year, new_month, 1,
                            )),
                        };
                    }

                    Some(date)
                }
            }
            Self::Annually(starting_date) => {
                if starting_date > &today {
                    Some(*starting_date)
                } else {
                    starting_date.with_year(starting_date.year() + 1)
                }
            }
        }
    }

    /// Returns the last day of the date's month
    fn get_last_date_of_month(date: NaiveDate) -> NaiveDate {
        NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
            .unwrap_or_else(|| NaiveDate::from_ymd(date.year() + 1, 1, 1))
            .pred()
    }

    fn next_date_by_day_of_month(today: NaiveDate, day: u32) -> NaiveDate {
        let last_date_this_month = Self::get_last_date_of_month(today);

        // gets the due date with the day argument. if the day doesn't exist for this
        // month, it returns the date of the last day of the month
        let due_date_this_month = today.with_day(day).unwrap_or(last_date_this_month);

        if due_date_this_month > today {
            due_date_this_month
        } else {
            // this modulus operation is a little confusing; we have to make sure the month is
            // zero-based, then add 2 months, mod so that the zero-based month wraps into next year
            // (if needed), then add 1 so that the month is one-based again
            let next_month_ordinal = ((today.month0() + 2) % 12) + 1;

            // this is kinda confusing too, but only adding 1 to today.month() will
            // lead to this month's last date. to get the last date of next month, we
            // have to add 2 to today.month()
            let last_date_next_month = NaiveDate::from_ymd_opt(today.year(), today.month() + 2, 1)
                .unwrap_or_else(|| NaiveDate::from_ymd(today.year() + 1, next_month_ordinal, 1))
                .pred();

            // return the date with the year and month of `last_date_next_month`, and try with the
            // day provided. if the date with `day` doesn't work, use `last_date_next_month`
            last_date_next_month
                .with_day(day)
                .unwrap_or(last_date_next_month)
        }
    }
}

impl Envelope {
    pub fn parse(
        mut chunk: &str,
        account_name: &str,
        decimal_symbol: char,
        date_format: &str,
    ) -> Result<Self, ParseError> {
        // trim the chunk to remove any unwanted \n
        chunk = chunk.trim();

        let mut lines = chunk.lines();

        let mut envelope = if let Some(l) = lines.next() {
            Self::from_header(l, date_format, account_name)?
        } else {
            let err = ParseError {
                context: Some(chunk.to_string()),
                message: Some(
                    "envelope header can't be parsed because it doesn't exist".to_string(),
                ),
            };
            return Err(err);
        };

        // parse the body
        let body_vec = lines.collect::<Vec<&str>>();
        let body = body_vec.join("\n");
        envelope.add_body(&body, account_name, decimal_symbol)?;

        Ok(envelope)
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Returns the starting struct of an Envelope. The string passed in can include ledger
    /// comments.
    fn from_header(
        mut header: &str,
        date_format: &str,
        account_name: &str,
    ) -> Result<Self, ParseError> {
        let tokens = utils::remove_comments(header)
            .trim()
            .split_whitespace()
            .collect::<Vec<&str>>();

        if tokens.len() < 2 {
            return Err(ParseError {
                context: Some(header.to_string()),
                message: Some("blank envelope header".to_string()),
            });
        }

        let envelope_type = match EnvelopeType::from_str(&tokens[0]) {
            Ok(t) => t,
            Err(e) => return Err(e),
        };

        // parse `starting` clause
        let (starting_date, starting_idx) = match Self::extract_starting(header, date_format) {
            Ok(o) => match o {
                Some(t) => (Some(t.0), Some(t.1)),
                None => (None, None),
            },
            Err(e) => return Err(e),
        };

        // if starting exists, trim the string supplied so that `starting` is cut off
        if let Some(i) = starting_idx {
            header = &header[..i];
        }

        let freq = match Self::extract_frequency(header, date_format, starting_date) {
            Ok(f) => f,
            Err(e) => return Err(e),
        };

        let envelope = Envelope {
            name: String::from(tokens[1]),
            amount: Amount::zero(),
            funding: FundingMethod::Manual,
            envelope_type,
            freq,
            auto_accounts: HashSet::new(),
            next_amount: Amount::zero(),
            now_amount: Amount::zero(),
            parent_account: String::from(account_name),
            starting_date,
            last_transaction_date: NaiveDate::from_ymd(0, 1, 1),
        };
        Ok(envelope)
    }

    fn add_body(
        &mut self,
        body: &str,
        account_name: &str,
        decimal_symbol: char,
    ) -> Result<(), ParseError> {
        for line in body.lines() {
            let trimmed_line = utils::remove_comments(line).trim();
            let line_split = trimmed_line.split_whitespace().collect::<Vec<&str>>();
            let key = line_split[0];

            // get the index of the first space (because that's where the values begins)
            let idx;
            match trimmed_line.find(' ') {
                Some(i) => idx = i,
                None => {
                    let message = format!(
                        "the property `{}` to an envelope (`{}` in {}) is blank",
                        line_split[0], self.name, account_name
                    );
                    let err = ParseError {
                        message: Some(message),
                        context: None,
                    };
                    return Err(err);
                }
            }

            let value = &trimmed_line[idx..].to_string();

            if line_split.len() > 1 {
                match key {
                    "for" => {
                        // parse a `for` property, which should only include an account (no spaces,
                        // of course)
                        if let Err(e) = self.add_account(value) {
                            return Err(e);
                        }
                    }
                    "amount" => {
                        // set the amount of the envelope
                        match Amount::parse(value, decimal_symbol) {
                            Ok(a) => {
                                self.amount = a;
                                self.next_amount.symbol = self.amount.symbol.clone();
                                self.now_amount.symbol = self.amount.symbol.clone();
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    "funding" => {
                        // parse the funding method for the envelope
                        match FundingMethod::from_str(value) {
                            Ok(f) => self.funding = f,
                            Err(e) => return Err(e),
                        }
                    }
                    _ => {
                        return Err(ParseError {
                            message: Some(format!(
                                "the `{}` property isn't understood by silverfox",
                                key
                            )),
                            context: None,
                        })
                    }
                }
            }
        }

        Ok(())
    }

    fn add_account(&mut self, s: &str) -> Result<(), ParseError> {
        let arg_count = s.split_whitespace().count();

        match arg_count.cmp(&1) {
            Ordering::Greater => {
                // more than one token? account probably has spaces in it
                Err(ParseError {
                    message: Some("remember that account names can't contain spaces; this `for` property couldn't be parsed correctly".to_string()),
                    context: Some(s.to_string()),
                })
            }
            Ordering::Less => {
                // something less than one token? that's an issue
                Err(ParseError {
                    message: Some("a `for` property is blank".to_string()),
                    context: Some(s.to_string()),
                })
            }
            Ordering::Equal => {
                // exactly one token? perfect
                self.auto_accounts.insert(String::from(s.trim()));
                Ok(())
            }
        }
    }

    fn extract_frequency(
        header: &str,
        date_format: &str,
        starting_date: Option<NaiveDate>,
    ) -> Result<Frequency, ParseError> {
        let clean_header = utils::remove_comments(header);
        if clean_header.contains("no date") {
            return Ok(Frequency::Never);
        }

        let frequency_index;
        // first try matching by "by", since it will always come after "by" whether "by" or "due by"
        match clean_header.rfind(" by ") {
            Some(i) => {
                frequency_index = i + " by ".len();
            },
            // if not found, search for "due"
            None => match clean_header.rfind(" due ") {
                Some(i) => {
                    frequency_index = i + " due ".len();
                },
                // if that's not found, then pbpbpbpbpbpbpbpbpbp
                None => return Err(ParseError {
                    message: Some("couldn't figure out when this envelope is due; use `no date` if you don't want to specify a due date".to_string()),
                    context: Some(clean_header.to_string())
                })
            }
        }

        let raw_freq = &clean_header[frequency_index..];
        Frequency::parse(raw_freq, date_format, starting_date)
    }

    /// Extracts and parses the `starting` clause of the Envelope. The Result returned uses an
    /// Option because a `starting` may exist or not exist.
    fn extract_starting(
        s: &str,
        date_format: &str,
    ) -> Result<Option<(NaiveDate, usize)>, ParseError> {
        let starting_idx = match s.find(" starting ") {
            Some(i) => i,
            None => return Ok(None),
        };

        let date_idx = starting_idx + " starting ".len();

        match NaiveDate::parse_from_str(&s[date_idx..], date_format) {
            Ok(d) => {
                let result = Some((d, starting_idx));
                Ok(result)
            }
            Err(_) => {
                let message = format!(
                    "couldn't parse starting date `{}` with format `{}`",
                    s, date_format
                );

                Err(ParseError {
                    message: Some(message),
                    context: Some(s.to_string()),
                })
            }
        }
    }

    fn make_bar(&self, amt: &Amount, width: usize) -> String {
        let width_f = width as f64;
        let progress = (amt.mag * width_f / self.amount.mag).min(width_f).max(0.0) as usize;
        let trough = width - progress;
        format!("|{}{}|", "═".repeat(progress), " ".repeat(trough))
    }

    fn make_text_progress(&self, amt: &Amount) -> String {
        format!("{} / {}", amt, self.amount)
    }

    /// Reads the Entry and makes changes to the envelope's balances (depending on accounts, dates,
    /// and amounts), as well as the envelope's last_entry_date
    pub fn process_entry(&mut self, entry: &Entry) -> Result<(), ProcessingError> {
        if entry.has_envelope_posting() {
            self.process_manual_postings(entry);
            Ok(())
        } else {
            self.infer(entry)
        }
    }

    fn process_manual_postings(&mut self, entry: &Entry) {
        // manual envelopes
        for posting in entry.get_envelope_postings() {
            // process each envelope posting in the entry

            if let Posting::Envelope(envelope_posting) = posting {
                // this posting can only apply if the accounts match
                if envelope_posting.get_account_name() == &self.parent_account
                    && &self.name == envelope_posting.get_envelope_name()
                {
                    // now, everything depends on the amount and date

                    let amount = envelope_posting.get_amount();

                    self.apply_amount(&amount, *entry.get_date());
                }
            }
        }
    }

    fn infer(&mut self, entry: &Entry) -> Result<(), ProcessingError> {
        // attempt to infer. silverfox can infer when postings for the account of the envelope and
        // *exactly one* of its `auto_accounts` exist
        //
        // otherwise, silverfox will throw an error and manual intervention is required

        // we should first check that at least one auto account and at least one self account
        // exists
        let mut self_account_count = 0;
        let mut auto_account_count = 0;

        // count
        for posting in entry.get_postings() {
            if *posting.get_account() == self.parent_account {
                self_account_count += 1;
            } else if self.auto_accounts.contains(posting.get_account()) {
                auto_account_count += 1;
            }
        }

        // check
        if self_account_count < 1 || auto_account_count < 1 {
            // if inference can't continue because of lack of the proper accounts, that's okay
            return Ok(());
        }

        // initialize sums
        let mut auto_postings_sum = Amount {
            mag: 0.0,
            symbol: self.amount.symbol.clone(),
        };
        let mut self_account_postings_sum = Amount {
            mag: 0.0,
            symbol: self.amount.symbol.clone(),
        };

        // calculate sums for envelope
        for posting in entry.get_postings() {
            let mut amount_to_add = match posting.get_amount() {
                Some(a) => a.clone(),
                None => match entry.get_blank_amount() {
                    Ok(o) => {
                        if let Some(b) = o {
                            b
                        } else {
                            unreachable!()
                        }
                    }
                    Err(e) => return Err(e),
                },
            };

            // if symbols don't match, try converting to native currency
            if amount_to_add.symbol != self.amount.symbol {
                // if this envelope's currency isn't blank (native), then nothing can happen here
                // because the currency can't be converted to native
                if self.amount.symbol.is_some() {
                    // can't infer because the envelope has a foreign currency, and this posting
                    // can't be converted to it
                    let message = format!(
"the envelope `{}` in `{}` was set up with a currency that isn't your native
currency. furthermore, this entry contains postings with accounts that relate to
the envelope, but silverfox could not move money automatically because the
postings use currencies that cannot be converted to the currency of the
envelope. hopefully that all makes sense!", self.name, self.parent_account);

                    return Err(ProcessingError {
                        message: Some(message),
                        context: Some(entry.as_full_string()),
                    });
                } else {
                    match posting.get_original_native_value() {
                        Some(m) => {
                            amount_to_add.mag = m;
                        },
                        None => {
                            return Err(ProcessingError::default()
                                .set_message(
"silverfox wants to infer how much money to move to or from an envelope, but
can't; you'll need to specify a manual envelope posting here with the correct
amount")
                                .set_context(entry.as_full_string().as_str())
                            )
                        }
                    }
                }
            }

            if posting.get_account() == &self.parent_account {
                self_account_postings_sum += amount_to_add;
            } else if self.auto_accounts.contains(posting.get_account()) {
                auto_postings_sum += amount_to_add;
            }
        }

        // the minimum of the absolute values of auto_postings_sum and self_account_postings_sum
        let abs_min_mag = auto_postings_sum
            .mag
            .min(self_account_postings_sum.mag.abs());

        // only apply an amount if the magnitude to add is worth something
        if abs_min_mag != 0.0 {
            // if the self_account_postings_sum is less than zero, then the amount we apply should be
            // negative
            let mag_to_apply = if self_account_postings_sum.mag < 0.0 {
                -abs_min_mag
            } else {
                abs_min_mag
            };

            self.apply_amount(
                &Amount {
                    mag: mag_to_apply,
                    symbol: self.amount.symbol.clone(),
                },
                *entry.get_date(),
            );
        }

        // done!
        Ok(())
    }

    fn apply_amount(&mut self, amount: &Amount, date: NaiveDate) {
        if amount.mag < 0.0 {
            // take from an envelope. always take from the 'now' envelope
            self.now_amount += amount.clone();
        } else if amount.mag > 0.0 {
            // add to an envelope, depending on the date
            if let Some(d) = self.freq.get_last_due_date() {
                if date < d {
                    // anything before the last due date is ready
                    self.now_amount += amount.clone();
                } else {
                    // otherwise, anything after the last due date is for the next due
                    // date
                    self.next_amount += amount.clone();
                }
            } else {
                // if no last due date, then everything is for next
                self.next_amount += amount.clone();
            }
        }

        self.last_transaction_date = date;
    }

    pub fn get_type(&self) -> &EnvelopeType {
        &self.envelope_type
    }

    fn get_total_amount_mag(&self) -> f64 {
        self.now_amount.mag + self.next_amount.mag
    }

    fn get_filling_amount(&self, account_available_amount: &Amount) -> Amount {
        assert_eq!(account_available_amount.symbol, self.amount.symbol);

        // some convenience variables
        let symbol = &self.amount.symbol;
        let zero_amount = Amount {
            mag: 0.0,
            symbol: symbol.clone(),
        };
        let next_due_date = if let Some(d) = self.get_next_due_date() {
            d
        } else {
            // no due date, no amount
            return zero_amount;
        };

        let today = Local::today().naive_utc();
        let remaining_amount = self.get_remaining_next_amount();

        if self.last_transaction_date == today {
            zero_amount
        } else {
            match self.funding {
                FundingMethod::Manual => {
                    // no automatic movement
                    zero_amount
                }
                FundingMethod::Aggressive => {
                    let mag = self
                        .amount
                        .mag
                        .min(account_available_amount.mag) // makes sure the account value stays positive :)
                        .min(remaining_amount.mag) // prevents envelope overflow
                        .max(-self.get_total_amount_mag()) // makes sure there are no negative envelope balances
                        .max(0.0); // never take money from an envelope

                    Amount {
                        mag,
                        symbol: symbol.clone(),
                    }
                }
                FundingMethod::Conservative => {
                    // get days remaining, and remaining amount
                    let date_diff = next_due_date.signed_duration_since(today);
                    let days_remaining = date_diff.num_days();
                    let mag = (remaining_amount.mag / days_remaining as f64)
                        .min(account_available_amount.mag) // makes sure the account value stays positive
                        .min(remaining_amount.mag) // prevents envelope overflow
                        .max(-self.get_total_amount_mag()) // makes sure there are no negative envelope balances
                        .max(0.0); // never take money from an envelope

                    // return that
                    Amount {
                        mag,
                        symbol: symbol.clone(),
                    }
                }
            }
        }
    }

    /// Returns a posting with this Envelope's fill amount for the day. `account` is passed so that
    /// the program can determine how much money we have available.
    pub fn get_filling_posting(&self, account_available_value: &AmountPool) -> EnvelopePosting {
        let amount = self.get_filling_amount(&account_available_value.only(&self.amount.symbol));

        EnvelopePosting::new(self.parent_account.clone(), amount, self.name.clone())
    }

    fn get_remaining_next_amount(&self) -> Amount {
        self.amount.clone() - self.next_amount.clone()
    }

    pub fn get_next_amount(&self) -> &Amount {
        &self.next_amount
    }

    pub fn get_now_amount(&self) -> &Amount {
        &self.now_amount
    }

    fn get_next_due_date(&self) -> Option<NaiveDate> {
        let starting_date = if let Some(d) = self.starting_date {
            d
        } else {
            return self.freq.get_next_due_date();
        };

        let freq_next_date = if let Some(d) = self.freq.get_next_due_date() {
            d
        } else {
            return Some(starting_date);
        };

        Some(starting_date.max(freq_next_date))
    }

    pub fn get_freq(&self) -> &Frequency {
        &self.freq
    }
}

impl fmt::Display for Envelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        let progress_bar_width = 40;

        // get next stuff
        let next_display = Amount {
            mag: self.next_amount.mag + self.now_amount.mag.min(0.0), // if now amount is below zero, subtract overflow from the next amount
            symbol: self.next_amount.symbol.clone(),
        };
        let next_prelude = if let Some(d) = self.get_next_due_date() {
            format!("next (on {})", d)
        } else {
            "next".to_string()
        };
        let next_text = self.make_text_progress(&next_display);
        let next_bar = self.make_bar(&next_display, 40);

        // get now stuff
        let now_display = Amount {
            mag: self.now_amount.mag.max(0.0), // will only be as small as zero (anything negative is taken from 'next')
            symbol: self.now_amount.symbol.clone(),
        };
        let now_text = self.make_text_progress(&now_display);
        let now_bar = self.make_bar(&now_display, progress_bar_width);

        writeln!(f, "    {}", self.name)?;
        writeln!(f, "      {:20} {:>30} {}", "now", now_text, now_bar)?;
        write!(
            f,
            "      {:20} {:>30} {}",
            next_prelude, next_text, next_bar
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtract_months_test() {
        let date_0 = NaiveDate::from_ymd(2019, 8, 2);
        let subtracted_0 = Frequency::subtract_months(date_0, 3);
        assert_eq!(NaiveDate::from_ymd(2019, 5, 2), subtracted_0);

        let date_1 = NaiveDate::from_ymd(2020, 1, 1);
        let subtracted_1 = Frequency::subtract_months(date_1, 3);
        assert_eq!(NaiveDate::from_ymd(2019, 10, 1), subtracted_1);
    }
}
