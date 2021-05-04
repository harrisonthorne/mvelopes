use crate::{entry::builder::EntryBuilder, entry::EntryStatus, errors::ParseError};
use chrono::NaiveDate;
use nom::character::complete::space1;
use nom::combinator::map;
use nom::multi::many1;
use nom::sequence::separated_pair;
use nom::{
    bytes::complete::is_not, character::complete::char, character::complete::one_of,
    character::complete::space0, combinator::map_res, combinator::opt, multi::separated_list1,
    sequence::delimited, sequence::pair, sequence::preceded, sequence::tuple, IResult,
};

use super::{eol_comment, parse_posting};

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

        let (input, _entry_heading_line_comment) = opt(preceded(space0, eol_comment))(input).map_err(|e| e.map(|_| ParseError {
            context: Some(input.to_string()),
            message: Some("tried to parse a comment, found something else".to_string()),
        }))?;

        // parses list of postings
        let posting_list = |input| {
            let posting_line = separated_pair(preceded(space1, parse_posting(decimal_symbol)), space0, opt(eol_comment));

            // for now, toss away comments when parsing postings
            many1(map(posting_line, |(p, _c)| p))(input).map_err(|e| e.map(|_| ParseError {
                context: Some(input.to_string()),
                message: Some(String::from("at least one posting is needed for entries")),
            }))
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
        map_res(preceded(space0, is_not("?~*\r\n")), |s| {
            NaiveDate::parse_from_str(s, date_format)
        })(input)
    }
}

fn parse_status(input: &str) -> IResult<&str, EntryStatus, ParseError> {
    map_res(preceded(space0, one_of("?~*")), EntryStatus::from_char)(input)
}

fn parse_description(input: &str) -> IResult<&str, &str, ParseError> {
    preceded(space0, is_not("\r\n]"))(input)
}

fn parse_payee(input: &str) -> IResult<&str, Option<&str>, ParseError> {
    opt(preceded(
        space0,
        delimited(char('['), is_not("\n\r"), char(']')),
    ))(input)
}
