use super::{eol_comment, parse_posting};
use crate::{
    entry::{builder::EntryBuilder, EntryStatus},
    errors::ParseError,
};
use chrono::NaiveDate;
use nom::{
    bytes::complete::is_not,
    character::complete::{char, multispace1, one_of, space0},
    combinator::{map, map_res, opt},
    multi::many1,
    sequence::{delimited, preceded, separated_pair, tuple},
    IResult,
};

fn parse_entry<'a>(
    date_format: &'a str,
    decimal_symbol: char,
) -> impl FnMut(&'a str) -> IResult<&'a str, EntryBuilder, ParseError> {
    move |input| {
        // parse heading
        let (input, (date, status, description, payee)) = tuple((
            parse_date(date_format),
            parse_status,
            parse_description,
            parse_payee,
        ))(input)?;

        let (input, _entry_heading_line_comment) = opt(preceded(space0, eol_comment))(input)
            .map_err(|e| {
                e.map(|_| ParseError {
                    context: Some(input.to_string()),
                    message: Some("tried to parse a comment, found something else".to_string()),
                })
            })?;

        // parses list of postings
        let posting_list = |input| {
            let posting_line = separated_pair(
                preceded(multispace1, parse_posting(decimal_symbol)),
                space0,
                opt(eol_comment),
            );

            // for now, toss away comments when parsing postings
            many1(map(posting_line, |(p, _)| p))(input).map_err(|e| {
                eprintln!("{}", e);
                e.map(|_| ParseError {
                    context: Some(input.to_string()),
                    message: Some(String::from("at least two postings are needed for entries")),
                })
            })
        };

        let (input, postings) = posting_list(input)?;

        let entry_builder = EntryBuilder::new()
            .date(date)
            .status(status)
            .description(description.to_string())
            .payee(payee.map(String::from))
            .postings(postings);

        Ok((input, entry_builder))
    }
}

fn parse_date<'a>(
    date_format: &'a str,
) -> impl FnMut(&'a str) -> IResult<&'a str, NaiveDate, ParseError> {
    move |input| {
        map_res(preceded(space0, is_not("?~*\r\n")), |s: &str| {
            NaiveDate::parse_from_str(s.trim(), date_format.trim())
        })(input)
    }
}

fn parse_status(input: &str) -> IResult<&str, EntryStatus, ParseError> {
    map_res(preceded(space0, one_of("?~*")), EntryStatus::from_char)(input)
}

fn parse_description(input: &str) -> IResult<&str, &str, ParseError> {
    map(preceded(space0, is_not("\r\n[];/")), |s: &str| s.trim())(input)
}

fn parse_payee(input: &str) -> IResult<&str, Option<&str>, ParseError> {
    opt(preceded(
        space0,
        delimited(char('['), is_not("\n\r]"), char(']')),
    ))(input)
}

#[cfg(test)]
mod tests {
    use crate::amount::Amount;
    use crate::posting::ClassicPosting;
    use crate::posting::EnvelopePosting;
    use crate::posting::Posting;

    use super::*;

    const ENTRY_ONE: &str = "2019/08/02 * Groceries [Grocery store]
    assets:checking    -50
    expenses:groceries";

    #[test]
    fn test_entry_one() {
        let (input, entry_builder) = parse_entry("%Y/%m/%d", '.')(ENTRY_ONE).unwrap();
        assert_eq!(
            entry_builder,
            EntryBuilder::new()
                .date(NaiveDate::from_ymd(2019, 8, 2))
                .status(EntryStatus::Reconciled)
                .description("Groceries".to_string())
                .payee(Some("Grocery store".to_string()))
                .posting(Posting::Classic(ClassicPosting::new(
                    "assets:checking",
                    Some(Amount {
                        mag: -50.0,
                        symbol: None
                    }),
                    None,
                    None,
                )))
                .posting(Posting::Classic(ClassicPosting::new(
                    "expenses:groceries",
                    None,
                    None,
                    None,
                )))
        );
        assert_eq!(input, "");
    }

    const ENTRY_TWO: &str = "2019.08.02 ~ Groceries with cash back ; a semicolon comment
    assets:checking                -70
    assets:cash                     20
    expenses:groceries              50
    envelope food assets:checking  -50";

    #[test]
    fn test_entry_two() {
        let (input, entry_builder) = parse_entry("%Y.%m.%d", '.')(ENTRY_TWO).unwrap();
        assert_eq!(
            entry_builder,
            EntryBuilder::new()
                .date(NaiveDate::from_ymd(2019, 8, 2))
                .status(EntryStatus::Cleared)
                .description("Groceries with cash back".to_string())
                .posting(Posting::Classic(ClassicPosting::new(
                    "assets:checking",
                    Some(Amount {
                        mag: -70.0,
                        symbol: None
                    }),
                    None,
                    None,
                )))
                .posting(Posting::Classic(ClassicPosting::new(
                    "assets:cash",
                    Some(Amount {
                        mag: 20.0,
                        symbol: None
                    }),
                    None,
                    None,
                )))
                .posting(Posting::Classic(ClassicPosting::new(
                    "expenses:groceries",
                    Some(Amount {
                        mag: 50.0,
                        symbol: None
                    }),
                    None,
                    None,
                )))
                .posting(Posting::Envelope(EnvelopePosting::new(
                    "assets:checking",
                    Amount {
                        mag: -50.0,
                        symbol: None
                    },
                    "food",
                )))
        );
        assert_eq!(input, "");
    }

    const ENTRY_THREE: &str = "2019-08-02 ? Bought crypto
    assets:checking     $-100 // a wild comment appeared!
    assets:crypto:btc       0.012345 BTC
    // oh no! extra input!";

    #[test]
    fn test_entry_three() {
        let (input, entry_builder) = parse_entry("%Y-%m-%d", '.')(ENTRY_THREE).unwrap();
        assert_eq!(
            entry_builder,
            EntryBuilder::new()
                .date(NaiveDate::from_ymd(2019, 8, 2))
                .status(EntryStatus::Pending)
                .description("Bought crypto".to_string())
                .posting(Posting::Classic(ClassicPosting::new(
                    "assets:checking",
                    Some(Amount {
                        mag: -100.0,
                        symbol: Some("$".to_string())
                    }),
                    None,
                    None,
                )))
                .posting(Posting::Classic(ClassicPosting::new(
                    "assets:crypto:btc",
                    Some(Amount {
                        mag: 0.012345,
                        symbol: Some("BTC".to_string())
                    }),
                    None,
                    None,
                )))
        );
        assert_eq!(input, "\n    // oh no! extra input!");
    }
}
