use nom::branch::permutation;
use nom::combinator::map;
use nom::{
    branch::alt, bytes::complete::is_not, bytes::complete::tag, character::complete::space0,
    character::complete::space1, combinator::opt, error::ErrorKind, sequence::pair,
    sequence::preceded, IResult,
};

use super::{common::eol_comment, parse_amount};

use crate::amount::Amount;
use crate::{
    errors::ParseError,
    posting::{ClassicPosting, Cost, EnvelopePosting, Posting},
};

/// Returns the leftover string and the Posting parsed.
pub fn parse_posting(
    line: &'static str,
    decimal_symbol: char,
) -> IResult<&str, Posting, ParseError> {
    let original_line = line.to_string();

    // scrap comments
    let input = eol_comment(line).map(|t| t.0).unwrap_or_else(|_| line);

    let (input, first_token) =
        preceded(space0, is_not(" \t\n\r"))(input).map_err(|e: nom::Err<(&str, ErrorKind)>| {
            e.map(|_| ParseError {
                context: Some(original_line),
                message: Some("no posting information here".to_string()),
            })
        })?;

    if first_token == "envelope" {
        parse_envelope_posting_information(input, decimal_symbol)
            .map(|(l, p)| (l, Posting::from(p)))
    } else {
        parse_normal_posting_information(input, decimal_symbol).map(|(l, p)| (l, Posting::from(p)))
    }
}

/// Parses everything after `envelope` in an envelope posting.
fn parse_envelope_posting_information(
    input: &str,
    decimal_symbol: char,
) -> IResult<&str, EnvelopePosting, ParseError> {
    let (input, envelope_name) = preceded(space1, is_not(" \t\n\r"))(input)
        .map(|(rem, s)| (rem, String::from(s)))
        .map_err(|e: nom::Err<(&str, ErrorKind)>| {
            e.map(|_| ParseError {
                context: Some(String::from(input)),
                message: Some("probably missing an envelope name".to_string()),
            })
        })?;
    let (input, account_name) = preceded(space1, is_not(" \t\n\r"))(input)
        .map(|(rem, s)| (rem, String::from(s)))
        .map_err(|e: nom::Err<(&str, ErrorKind)>| e.map(|_|
            ParseError {
                context: Some(String::from(input)),
                message: Some("probably missing an account name. silverfox currently doesn't support implicit accounts in manual envelope postings".to_string()),
            }
        ))?;
    let (leftover, amount) = super::amount::parse_amount(input, decimal_symbol)?;

    Ok((
        leftover,
        EnvelopePosting::new(account_name, amount, envelope_name),
    ))
}

/// Parses everything after the account name in a class posting.
fn parse_normal_posting_information(
    input: &str,
    decimal_symbol: char,
) -> IResult<&str, ClassicPosting, ParseError> {
    let orig = input.clone();

    let (input, account) =
        preceded(space0, is_not(" \t\n\r"))(input).map_err(|e: nom::Err<(&str, ErrorKind)>| {
            e.map(|_| ParseError {
                context: Some(input.to_string()),
                message: Some(
                    "an account name couldn't be found (or parsed for that matter)".to_string(),
                ),
            })
        })?;

    let (input, amount) = opt(|inp| parse_amount(inp, decimal_symbol))(input).map_err(|e| e.map(|_| ParseError {
        context: Some(input.to_string()),
        message: Some(format!("an issue occurred when trying to parse an amount here.\nthis probably isn't supposed to happen. here's some extra info on this error: {}", e)),
    }))?;

    let (leftover, (cost_assertion, balance_assertion)) = permutation((
        opt(|inp| parse_cost_assertion(inp, decimal_symbol)),
        opt(|inp| parse_balance_assertion(inp, decimal_symbol)),
    ))(input)?;

    Ok((
        leftover,
        ClassicPosting {
            amount,
            account: account.to_string(),
            cost_assertion,
            balance_assertion,
        },
    ))
}

fn parse_cost_assertion(input: &str, decimal_symbol: char) -> IResult<&str, Cost, ParseError> {
    let by_unit = map(
        preceded(pair(alt((tag("@"), tag("each"))), space1), |inp| {
            parse_amount(inp, decimal_symbol)
        }),
        Cost::UnitCost,
    );

    let by_total = map(
        preceded(
            pair(alt((tag("@@"), tag("=="), tag("totaling"))), space1),
            |inp| parse_amount(inp, decimal_symbol),
        ),
        Cost::TotalCost,
    );

    alt((by_unit, by_total))(input).map_err(|e| {
        e.map(|_| ParseError {
            context: Some(input.to_string()),
            message: Some("couldn't parse this as a cost assertion".to_string()),
        })
    })
}

fn parse_balance_assertion(input: &str, decimal_symbol: char) -> IResult<&str, Amount, ParseError> {
    preceded(pair(alt((tag("!"), tag("="), tag("bal"))), space1), |inp| {
        parse_amount(inp, decimal_symbol)
    })(input)
}
