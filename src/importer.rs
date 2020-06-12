use crate::entry::{Entry, EntryStatus};
use crate::errors::*;
use crate::posting::{Posting, ClassicPosting};
use crate::utils;
use std::collections::{HashMap, HashSet};
use std::collections::{LinkedList, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

pub struct CsvImporter {
    rules: Rules,
    records: VecDeque<csv::StringRecord>,
    ledger_account_set: HashSet<String>,
}

impl CsvImporter {
    pub fn from_file(
        csv_file: impl AsRef<Path>,
        ledger_account_set: HashSet<String>,
    ) -> Result<Self, MvelopesError> {
        let rules_file = Self::get_sibling_rules_path(&csv_file);

        Self::from_file_with_rules(csv_file, rules_file, ledger_account_set)
    }

    fn get_sibling_rules_path(original: impl AsRef<Path>) -> PathBuf {
        let mut os_string = original.as_ref().to_path_buf().into_os_string();
        os_string.push(".rules");

        PathBuf::from(os_string)
    }

    pub fn from_file_with_rules(
        csv_file: impl AsRef<Path>,
        rules_file: impl AsRef<Path>,
        ledger_account_set: HashSet<String>,
    ) -> Result<Self, MvelopesError> {
        let csv_str = fs::read_to_string(&csv_file).map_err(|e| MvelopesError::file_error(&csv_file, e))?;
        let rules_str = fs::read_to_string(&rules_file).map_err(|e| MvelopesError::file_error(&rules_file, e))?;

        Self::from_strs(&csv_str, &rules_str, ledger_account_set)
    }

    fn from_strs(
        csv_str: &str,
        rules_str: &str,
        ledger_account_set: HashSet<String>,
    ) -> Result<Self, MvelopesError> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(csv_str.as_bytes());

        let rules = Rules::from_str(rules_str)?;
        let mut records: VecDeque<csv::StringRecord> = VecDeque::new();
        for result in reader.records().skip(rules.skip as usize) {
            match result {
                Ok(r) => records.push_back(r),
                Err(e) => {
                    return Err(MvelopesError::from(ParseError {
                        message: Some(format!("there was an error reading csv records: {}", e)),
                        context: None,
                    }))
                }
            }
        }

        Ok(Self {
            rules,
            records,
            ledger_account_set,
        })
    }
}

impl Iterator for CsvImporter {
    type Item = Result<Entry, MvelopesError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.records.pop_front() {
            None => None,
            Some(r) => {
                Some(
                    self.rules
                        .get_entry_from_record(&r, &self.ledger_account_set.iter().collect()),
                ) // blech
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Rules {
    accounts: HashMap<String, String>,
    amount_strs: HashMap<String, String>,
    comment: String,
    description: String,
    date_format: String,
    date_str: String,
    decimal_symbol: char,
    fields: LinkedList<String>,
    payee: String,
    skip: i32,
    status: String,
    subrules: Vec<Subrules>,
}

impl Default for Rules {
    fn default() -> Self {
        let amount_strs: HashMap<String, String> = Default::default();
        let accounts: HashMap<String, String> = Default::default();

        Rules {
            accounts,
            amount_strs,
            comment: Default::default(),
            date_format: String::from("%Y/%m/%d"),
            date_str: String::from("%date%"),
            decimal_symbol: '.',
            description: String::from("%description%"),
            fields: Default::default(),
            payee: String::new(),
            skip: 1,
            status: String::from("~"),
            subrules: Default::default(),
        }
    }
}

impl Rules {
    pub fn from_str(rules_str: &str) -> Result<Self, MvelopesError> {
        let mut rules: Rules = Default::default();
        if let Err(e) = rules.add_from_str(rules_str) {
            Err(e)
        } else {
            Ok(rules)
        }
    }

    fn add_from_file(&mut self, rules_file: &Path) -> Result<(), MvelopesError> {
        let s = fs::read_to_string(rules_file).map_err(|e| MvelopesError::file_error(rules_file, e))?;

        self.add_from_str(&s)
    }

    fn add_from_str(&mut self, s: &str) -> Result<(), MvelopesError> {
        // Some if parsing a Subrules. None if parsing other things.
        let mut parsing_subrules: Option<Subrules> = None;
        let mut parsing_subrules_rules = false;

        for mut line in s.lines() {
            line = utils::remove_comments(line);

            // if line contains nothing, continue
            if line.trim().is_empty() {
                continue;
            }

            if let Some(s) = parsing_subrules.as_mut() {
                let mut chars = line.chars();
                if !parsing_subrules_rules {
                    // if parsing_subrules hasn't parsed Rules yet...
                    if let Some(c) = chars.next() {
                        if c.is_whitespace() {
                            // a line starting with whitespace is a rule, so the flag must be set
                            parsing_subrules_rules = true;
                            (*s).rules.add_from_line(line)?;
                        } else {
                            // a line starting with a non-whitespace character is a pattern to the
                            // Subrules
                            (*s).patterns.push(String::from(line));
                        }
                    }

                    // don't parse any more (more parsing from here will cause unwanted changes to
                    // the root rules)
                    continue;
                } else {
                    // if parsing_subrules has Rules, then...
                    if let Some(c) = chars.next() {
                        if c.is_whitespace() {
                            // a line starting with whitespace is a rule
                            (*s).rules.add_from_line(line)?;

                            // don't parse any more (more parsing from here will cause unwanted
                            // changes to the root rules)
                            continue;
                        } else {
                            // a line starting with a non-whitespace character ends this parsing of
                            // these subrules
                            self.subrules.push(s.clone());
                            parsing_subrules = None;
                            parsing_subrules_rules = false;

                            // this is the only place we don't use the continue keyword. since this
                            // line triggers an ending to parsing Subrules, it could be something
                            // to be parsed in the root Rules
                        }
                    }
                }
            }

            if line.starts_with("if") {
                parsing_subrules = Some(Subrules::from(&*self));
                if let Some(i) = line.chars().position(|c| c.is_whitespace()) {
                    match parsing_subrules.as_mut() {
                        Some(s) => (*s).patterns.push(String::from(&line[i + 1..])),
                        None => unreachable!(), // should be unreachable, as parsing_subrules was just initialized as Some
                    }
                }
            } else {
                self.add_from_line(line)?;
            }
        }

        // at the end, if parsing_subrules is Some, it needs to be added
        if let Some(r) = parsing_subrules {
            self.subrules.push(r);
        }

        Ok(())
    }

    fn add_from_line(&mut self, mut line: &str) -> Result<(), MvelopesError> {
        line = line.trim_start();

        let split_index =
            match line.chars().position(|c| c.is_whitespace()) {
                Some(i) => i,
                None => return Err(MvelopesError::from(ParseError {
                    message: Some(format!(
                        "this rule has no value. use `-` if you want to discard a value:\n\n{} -",
                        line.trim()
                    )),
                    context: Some(line.to_string()),
                })),
            };

        // the first token is the rule name
        let rule_name = &line[..split_index];

        // the rest of the line is the value
        let rule_value = String::from(&line[split_index + 1..]).trim_start().to_string();

        if rule_value.trim() == "-" {
            // resets a value
            match rule_name {
                "comment" | "note" => self.comment = String::new(),
                "date_format" => self.date_format = String::from("%Y/%m/%d"),
                "date" => self.date_str = String::from("%date%"),
                "decimal_symbol" | "decimal" => self.decimal_symbol = '.',
                "description" => self.description = String::from("%description%"),
                "fields" => {
                    return Err(MvelopesError::from(ValidationError {
                        message: Some(String::from("`fields` cannot be discarded; a value is required")),
                        context: None,
                    }))
                }
                "if" => {
                    // because root-level subrules should be handled by add_from_str, this should not
                    // be called unless rules are being added line by line, which is what happens when
                    // parsing Subrules
                    return Err(MvelopesError::from(
                        ParseError::default().set_message("nested subrules aren't allowed"),
                    ));
                }
                "include" | "use" => self.add_from_file(&PathBuf::from(rule_value))?,
                "payee" => self.payee = String::new(),
                "skip" => self.skip = 1,
                "status" => self.status = String::from("~"),
                _ => {
                    // attempt parsing an amount index or an account index
                    if rule_name.starts_with("amount") {
                        let index_str = &rule_name["amount".len()..];
                        self.amount_strs.remove(&String::from(index_str));
                    } else if rule_name.starts_with("account") {
                        let index_str = &rule_name["account".len()..];
                        self.accounts.remove(&String::from(index_str));
                    } else {
                        return Err(MvelopesError::from(ParseError {
                            message: Some(format!(
                                "`{}` is not a rule that mvelopes understands",
                                rule_name
                            )),
                            context: Some(line.to_string()),
                        }));
                    }
                }
            }
        } else {
            // sets a value
            match rule_name {
                "comment" | "note" => self.comment = rule_value,
                "date_format" => self.date_format = rule_value,
                "date" => self.date_str = rule_value,
                "decimal_symbol" | "decimal" => {
                    if rule_value.len() > 1 {
                        return Err(MvelopesError::from(
                            ParseError::default()
                                .set_message("decimal_symbol should be a single character")
                                .set_context(line),
                        ));
                    } else {
                        self.decimal_symbol = rule_value.chars().next().unwrap();
                    }
                }
                "description" => self.description = rule_value,
                "fields" => {
                    for field_name in rule_value.split(',') {
                        self.fields.push_back(String::from(field_name.trim()))
                    }
                }
                "if" => {
                    // because root-level subrules should be handled by add_from_str, this should not
                    // be called unless rules are being added line by line, which is what happens when
                    // parsing Subrules
                    return Err(MvelopesError::from(
                        ParseError::default().set_message("nested subrules aren't allowed"),
                    ));
                }
                "include" | "use" => self.add_from_file(&PathBuf::from(rule_value))?,
                "payee" => self.payee = rule_value,
                "skip" => {
                    self.skip =
                        match rule_value.parse::<i32>() {
                            Ok(n) => n,
                            Err(e) => return Err(MvelopesError::from(ParseError {
                                message: Some(format!(
                                    "the `skip` rule couldn't be parsed because of this error: {}",
                                    e
                                )),
                                context: None,
                            })),
                        }
                }
                "status" => {
                    self.status = rule_value;
                }
                _ => {
                    // attempt parsing an amount index or an account index
                    if rule_name.starts_with("amount") {
                        let index_str = &rule_name["amount".len()..];
                        self.amount_strs.insert(String::from(index_str), rule_value);
                    } else if rule_name.starts_with("account") {
                        let index_str = &rule_name["account".len()..];
                        self.accounts.insert(String::from(index_str), rule_value);
                    } else {
                        return Err(MvelopesError::from(ParseError {
                            message: Some(format!(
                                "`{}` is not a rule that mvelopes understands",
                                rule_name
                            )),
                            context: Some(line.to_string()),
                        }));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_entry_from_record(
        &mut self,
        record: &csv::StringRecord,
        account_set: &HashSet<&String>,
    ) -> Result<Entry, MvelopesError> {
        // if any subrules apply to this record, use those rules instead
        for subrules in self.subrules.iter_mut() {
            if subrules.applies_to(record) {
                return subrules.rules.get_entry_from_record(record, account_set);
            }
        }
        // otherwise, continue on
    
        // if accounts are blank, add default
        if self.accounts.is_empty() {
            self.accounts.insert(String::from(""), String::from("%account%"));
        }

        // if accounts are blank, add default
        if self.amount_strs.is_empty() {
            self.amount_strs.insert(String::from(""), String::from("%amount%"));
        }

        // create the variables map by reading from the fields in the csv record
        let mut variables: HashMap<String, String> = HashMap::new();
        for (field_name, field_value) in self.fields.iter().zip(record.iter()) {
            // no duplicate variables are allowed
            if variables.contains_key(field_name) {
                return Err(MvelopesError::from(ParseError {
                    message: Some(format!(
                        "there is a duplicate field definition in your rules file: `{}`",
                        field_name
                    )),
                    context: None,
                }));
            }

            variables.insert(String::from(field_name), String::from(field_value));
        }

        // get date
        let raw_date = Self::inject_variables(&self.date_str, &variables);
        let date = match chrono::NaiveDate::parse_from_str(&raw_date, &self.date_format) {
            Ok(d) => d,
            Err(e) => {
                return Err(MvelopesError::from(ParseError {
                    message: Some(format!(
                        "there was an error parsing `{}` with the format `{}`: {}",
                        raw_date, self.date_format, e
                    )),
                    context: None,
                }))
            }
        };

        // get others
        let description = Self::inject_variables(&self.description, &variables);
        let status = Self::inject_variables(&self.status, &variables).parse::<EntryStatus>()?;

        // get payee
        let payee = if self.payee.trim().is_empty() {
            None
        } else {
            Some(Self::inject_variables(&self.payee, &variables))
        };

        //
        let comment = if self.comment.trim().is_empty() {
            None
        } else {
            Some(Self::inject_variables(&self.comment, &variables))
        };

        // make postings from account and amount sets
        let mut postings: Vec<Posting> = Vec::new();
        for (index, account_name) in self.accounts.iter() {
            let raw_value = match self.amount_strs.get(index) {
                Some(amount_str) => format!("{} {}", account_name, amount_str),
                None => account_name.clone(),
            };

            let injected = Self::inject_variables(&raw_value, &variables);

            match Posting::parse(injected.as_str(), self.decimal_symbol, account_set) {
                Ok(p) => postings.push(p),
                Err(e) => return Err(e),
            }
        }

        // reverse, because.
        postings.reverse();

        // validate number of postings
        match postings.len() {
            1 => {
                let single_posting_amount = postings[0].get_amount();
                if let Some(amount) = single_posting_amount {
                    if amount.mag < 0.0 {
                        postings.push(Posting::from(ClassicPosting::new("expenses:unknown", None, None, None, None)))
                    } else if amount.mag > 0.0 {
                        postings.push(Posting::from(ClassicPosting::new("income:unknown", None, None, None, None)))
                    } else {
                        // don't freak out about amounts with zero amounts
                        postings.push(Posting::from(ClassicPosting::new("unknown", None, None, None, None)))
                    }

                    Ok(Entry::new(date, status, description, payee, postings, comment))
                } else {
                    Err(MvelopesError::from(ValidationError::default().set_message("an entry with only one posting was generated, and that posting had a blank amount. make sure you've included an `amount` rule")))
                }
            },
            0 => {
                Err(MvelopesError::from(ValidationError::default().set_context(record.as_slice()).set_message("this record produced an entry without any postings. make sure you've included rules for `account` and `amount` so that postings can be generated")))
            },
            _ => {
                Ok(Entry::new(date, status, description, payee, postings, comment))
            }
        }
    }

    fn inject_variables(s: &str, variables: &HashMap<String, String>) -> String {
        let mut result = String::from(s);

        for (v_name, v_value) in variables.iter() {
            result = result.replace(format!("%{}%", v_name).as_str(), v_value);
        }

        result = result.replace("%%", "%"); // literal %

        result
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Subrules {
    patterns: Vec<String>,
    rules: Rules,
}

impl Subrules {
    fn applies_to(&self, record: &csv::StringRecord) -> bool {
        let s = record.as_slice().to_lowercase();

        self.patterns.iter().any(|p| s.contains(&p.to_lowercase()))
    }
}

impl From<&Rules> for Subrules {
    fn from(other: &Rules) -> Self {
        Self {
            patterns: Default::default(),
            rules: other.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::amount::Amount;
    use crate::posting::Cost;
    use super::*;

    const RULES_STR: &str = "fields date, description, amount, currency, native_price, other

amount %amount% %currency% @ %native_price%
account assets:test

decimal_symbol .

date_format %Y.%m.%d

comment test comment

skip 1

if test0
    comment single condition test

if test1
test2
test3
    comment multiple condition test
    payee Ferris the Crab

if bad decimal
test4
test5
    comment comma decimal_symbol test
    decimal_symbol ,";

    const CSV_STR: &str = "date,description,amount,currency,native_price,other
2020.10.09,Test CSV Entry One,1.2,BTC,11000,
2020.11.12,Test CSV Entry Two,-3.4,BTC,10000,test0
2020.12.13,Test CSV Entry Three,5.6,BTC,9000,test1
2020.01.02,Test CSV Entry Four,-7.8,BTC,8000,test2
2020.02.14,Test CSV Entry Five,\"9,1\",BTC,12000,bad decimal";

    #[test]
    fn sibling_rules_path_test() {
        let csv_path = PathBuf::from("csv_file.csv");
        let sibling = CsvImporter::get_sibling_rules_path(&csv_path);
        assert_eq!(sibling, PathBuf::from("csv_file.csv.rules"));
    }

    #[test]
    fn parse_rules_test() {
        let mut rules = Rules::default();
        let other = parse_rules_test_struct();
        if let Err(e) = rules.add_from_str(RULES_STR) {
            panic!("{}", e);
        } else {
            assert_eq!(rules, other);
        }
    }

    #[test]
    fn parse_csv_test() {
        let mut ledger_account_set = HashSet::<String>::new();
        ledger_account_set.insert(String::from("assets:test"));

        let importer = match CsvImporter::from_strs(CSV_STR, RULES_STR, ledger_account_set) {
            Ok(i) => i,
            Err(e) => panic!("{}", e),
        };
        let mut entries = Vec::<Entry>::new();

        for result in importer {
            match result {
                Ok(e) => entries.push(e),
                Err(e) => panic!("{}", e),
            }
        }

        assert_eq!(
            format!("{:?}", entries),
            format!("{:?}", parse_csv_test_entries())
        )
    }

    fn parse_csv_test_entries() -> Vec<Entry> {
        let mut entries = Vec::new();

        // entry 0
        let entry0: Entry;
        {
            let amount0 = Amount {
                mag: 1.2,
                symbol: Some(String::from("BTC")),
            };
            let price0 = Amount {
                mag: 11000.0,
                symbol: None,
            };
            let posting0_0 =
                Posting::from(ClassicPosting::new("assets:test", Some(amount0), Some(Cost::UnitCost(price0)), None, None));
            let posting0_1 = Posting::from(ClassicPosting::new("income:unknown", None, None, None, None));
            entry0 = Entry::new(
                chrono::NaiveDate::from_ymd(2020, 10, 9),
                EntryStatus::Cleared,
                String::from("Test CSV Entry One"),
                None,
                vec![posting0_0, posting0_1],
                Some(String::from("test comment")),
            );
            entries.push(entry0);
        }

        // entry 1
        let entry1: Entry;
        {
            let amount1 = Amount {
                mag: -3.4,
                symbol: Some(String::from("BTC")),
            };
            let price1 = Amount {
                mag: 10000.0,
                symbol: None,
            };
            let posting1_0 =
                Posting::from(ClassicPosting::new("assets:test", Some(amount1), Some(Cost::UnitCost(price1)), None, None));
            let posting1_1 = Posting::from(ClassicPosting::new("expenses:unknown", None, None, None, None));
            entry1 = Entry::new(
                chrono::NaiveDate::from_ymd(2020, 11, 12),
                EntryStatus::Cleared,
                String::from("Test CSV Entry Two"),
                None,
                vec![posting1_0, posting1_1],
                Some(String::from("single condition test")),
            );
            entries.push(entry1);
        }

        // entry 2
        let entry2: Entry;
        {
            let amount2 = Amount {
                mag: 5.6,
                symbol: Some(String::from("BTC")),
            };
            let price2 = Amount {
                mag: 9000.0,
                symbol: None,
            };
            let posting2_0 =
                Posting::from(ClassicPosting::new("assets:test", Some(amount2), Some(Cost::UnitCost(price2)), None, None));
            let posting2_1 = Posting::from(ClassicPosting::new("income:unknown", None, None, None, None));
            entry2 = Entry::new(
                chrono::NaiveDate::from_ymd(2020, 12, 13),
                EntryStatus::Cleared,
                String::from("Test CSV Entry Three"),
                Some(String::from("Ferris the Crab")),
                vec![posting2_0, posting2_1],
                Some(String::from("multiple condition test")),
            );
            entries.push(entry2);
        }

        // entry 3
        let entry3: Entry;
        {
            let amount3 = Amount {
                mag: -7.8,
                symbol: Some(String::from("BTC")),
            };
            let price3 = Amount {
                mag: 8000.0,
                symbol: None,
            };
            let posting3_0 =
                Posting::from(ClassicPosting::new("assets:test", Some(amount3), Some(Cost::UnitCost(price3)), None, None));
            let posting3_1 = Posting::from(ClassicPosting::new("expenses:unknown", None, None, None, None));
            entry3 = Entry::new(
                chrono::NaiveDate::from_ymd(2020, 1, 2),
                EntryStatus::Cleared,
                String::from("Test CSV Entry Four"),
                Some(String::from("Ferris the Crab")),
                vec![posting3_0, posting3_1],
                Some(String::from("multiple condition test")),
            );
            entries.push(entry3);
        }

        // entry 4
        let entry4: Entry;
        {
            let amount4 = Amount {
                mag: 9.1,
                symbol: Some(String::from("BTC")),
            };
            let price4 = Amount {
                mag: 12000.0,
                symbol: None,
            };
            let posting4_0 =
                Posting::from(ClassicPosting::new("assets:test", Some(amount4), Some(Cost::UnitCost(price4)), None, None));
            let posting4_1 = Posting::from(ClassicPosting::new("income:unknown", None, None, None, None));
            entry4 = Entry::new(
                chrono::NaiveDate::from_ymd(2020, 2, 14),
                EntryStatus::Cleared,
                String::from("Test CSV Entry Five"),
                None,
                vec![posting4_0, posting4_1],
                Some(String::from("comma decimal_symbol test")),
            );
            entries.push(entry4);
        }

        entries
    }

    fn parse_rules_test_struct() -> Rules {
        let mut rules: Rules = Default::default();

        rules.fields.push_back(String::from("date"));
        rules.fields.push_back(String::from("description"));
        rules.fields.push_back(String::from("amount"));
        rules.fields.push_back(String::from("currency"));
        rules.fields.push_back(String::from("native_price"));
        rules.fields.push_back(String::from("other"));
        rules.amount_strs.insert(
            String::from(""),
            String::from("%amount% %currency% @ %native_price%"),
        );
        rules
            .accounts
            .insert(String::from(""), String::from("assets:test"));
        rules.comment = String::from("test comment");
        rules.date_format = String::from("%Y.%m.%d");
        rules.decimal_symbol = '.';
        rules.skip = 1;

        let mut subrules_0 = Subrules::from(&rules);
        subrules_0.patterns.push(String::from("test0"));
        subrules_0.rules.comment = String::from("single condition test");
        rules.subrules.push(subrules_0);

        let mut subrules_1: Subrules = Subrules::from(&rules);
        subrules_1.patterns.push(String::from("test1"));
        subrules_1.patterns.push(String::from("test2"));
        subrules_1.patterns.push(String::from("test3"));
        subrules_1.rules.comment = String::from("multiple condition test");
        subrules_1.rules.payee = String::from("Ferris the Crab");
        rules.subrules.push(subrules_1);

        let mut bad_decimal_subrules: Subrules = Subrules::from(&rules);
        bad_decimal_subrules.rules.decimal_symbol = ',';
        bad_decimal_subrules
            .patterns
            .push(String::from("bad decimal"));
        bad_decimal_subrules.patterns.push(String::from("test4"));
        bad_decimal_subrules.patterns.push(String::from("test5"));
        bad_decimal_subrules.rules.comment = String::from("comma decimal_symbol test");
        rules.subrules.push(bad_decimal_subrules);

        rules
    }
}
