use super::{Entry, EntryStatus};
use crate::{errors::SilverfoxResult, errors::ValidationError, posting::Posting};
use chrono::NaiveDate;

#[derive(Default, Debug, Eq, PartialEq)]
pub struct EntryBuilder {
    date: Option<NaiveDate>,
    status: Option<EntryStatus>,
    description: Option<String>,
    payee: Option<String>,
    comment: Option<String>,
    postings: Vec<Posting>,
}

impl EntryBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn date(mut self, date: NaiveDate) -> Self {
        self.date = Some(date);
        self
    }

    pub fn status(mut self, status: EntryStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn payee(mut self, payee: Option<String>) -> Self {
        self.payee = payee;
        self
    }

    pub fn comment(mut self, comment: Option<String>) -> Self {
        self.comment = comment;
        self
    }

    pub fn posting(mut self, posting: Posting) -> Self {
        self.postings.push(posting);
        self
    }

    pub fn postings(mut self, postings: Vec<Posting>) -> Self {
        self.postings = postings;
        self
    }

    pub fn build(self) -> SilverfoxResult<Entry> {
        Ok(Entry {
            date: self.date.ok_or_else(|| ValidationError {
                context: None,
                message: Some(String::from("an entry is missing a date")),
            })?,
            status: self.status.ok_or_else(|| ValidationError {
                context: None,
                message: Some(String::from(
                    "silverfox requires statuses on entries and an entry was found without one",
                )),
            })?,
            description: self.description.ok_or_else(|| ValidationError {
                context: None,
                message: Some(String::from("a description is required for entries")),
            })?,
            payee: self.payee,
            comment: self.comment,
            postings: self.postings,
        })
    }
}
