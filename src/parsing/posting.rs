use nom::bytes::complete::tag;
use nom::sequence::tuple;
use nom::{
    branch::{alt, permutation},
    bytes::complete::is_not,
    character::complete::{space0, space1},
    combinator::{map, opt},
    error::ErrorKind,
    sequence::{pair, preceded},
    IResult,
};

use super::parse_amount;

use crate::{
    amount::Amount,
    errors::ParseError,
    posting::{ClassicPosting, Cost, EnvelopePosting, Posting},
};

/// Returns the leftover string and the Posting parsed.
pub fn parse_posting(
    decimal_symbol: char,
) -> impl FnMut(&str) -> IResult<&str, Posting, ParseError> {
    move |inp| {
        let original_line = inp.to_string();

        // precede with whitespace, just in case
        let (input, first_token) = preceded(space0, is_not(" \t\n\r"))(inp).map_err(
            |e: nom::Err<(&str, ErrorKind)>| {
                e.map(|_| ParseError {
                    context: Some(original_line),
                    message: Some("no posting information here".to_string()),
                })
            },
        )?;

        // the rest of the posting depends on the first token, which determines the type
        // -    "envelope" => it's an envelope posting
        // -    literally anything else, probably an account name => it's a classic
        //      posting
        if first_token == "envelope" {
            parse_envelope_posting_information(input, decimal_symbol)
                .map(|(l, p)| (l, Posting::from(p)))
        } else {
            parse_normal_posting_information(input, first_token, decimal_symbol)
                .map(|(l, p)| (l, Posting::from(p)))
        }
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
    let (leftover, amount) = super::amount::parse_amount(decimal_symbol)(input)?;

    Ok((
        &leftover,
        EnvelopePosting::new(&account_name, amount, &envelope_name),
    ))
}

/// Parses everything after the account name in a class posting.
fn parse_normal_posting_information<'a>(
    input: &'a str,
    account_name: &str,
    decimal_symbol: char,
) -> IResult<&'a str, ClassicPosting, ParseError> {
    let orig = input.to_string();

    let (input, amount) = opt(parse_amount(decimal_symbol))(input).map_err(|e| e.map(|e| ParseError {
        context: Some(input.to_string()),
        message: Some(format!("an issue occurred when trying to parse an amount here.\nthis probably isn't supposed to happen. here's some extra info on this error: {}", e)),
    }))?;

    // parses cost assertion and balance assertion, checking for either one, the other, or both
    let (leftover, (cost_assertion, balance_assertion)) = {
        let (leftover, assertions) = opt(alt((
            map(
                permutation((
                    parse_cost_assertion(decimal_symbol),
                    parse_balance_assertion(decimal_symbol),
                )),
                |(ca, ba)| (Some(ca), Some(ba)),
            ),
            map(parse_cost_assertion(decimal_symbol), |ca| (Some(ca), None)),
            map(parse_balance_assertion(decimal_symbol), |ba| {
                (None, Some(ba))
            }),
        )))(input)?;

        (leftover, assertions.unwrap_or((None, None)))
    };

    Ok((
        leftover,
        ClassicPosting::new(account_name, amount, cost_assertion, balance_assertion),
    ))
}

fn parse_cost_assertion(
    decimal_symbol: char,
) -> impl FnMut(&str) -> IResult<&str, Cost, ParseError> {
    move |input| {
        let by_unit = map(
            preceded(
                tuple((space0, tag("@"), space1)),
                parse_amount(decimal_symbol),
            ),
            Cost::UnitCost,
        );

        let by_total = map(
            preceded(
                tuple((space0, alt((tag("@@"), tag("=="))), space1)),
                parse_amount(decimal_symbol),
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
}

fn parse_balance_assertion(
    decimal_symbol: char,
) -> impl FnMut(&str) -> IResult<&str, Amount, ParseError> {
    move |input| {
        let tags = (tag("!"), tag("="));
        preceded(
            tuple((space0, alt(tags), space1)),
            parse_amount(decimal_symbol),
        )(input)
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_parse_posting() {
        // basic ho
        assert_eq!(
            parse_posting('.')("assets:cash 10"),
            Ok((
                "",
                Posting::Classic(ClassicPosting::new(
                    "assets:cash",
                    Some(Amount {
                        symbol: None,
                        mag: 10.0
                    }),
                    None,
                    None,
                ))
            ))
        );

        // commas as decimals
        assert_eq!(
            parse_posting(',')("assets:checking 123,45"),
            Ok((
                "",
                Posting::Classic(ClassicPosting::new(
                    "assets:checking",
                    Some(Amount {
                        symbol: None,
                        mag: 123.45
                    }),
                    None,
                    None,
                ))
            ))
        );

        // expensive food
        assert_eq!(
            parse_posting(',')("envelope food assets:cash BTC -50"),
            Ok((
                "",
                Posting::Envelope(EnvelopePosting::new(
                    "assets:cash",
                    Amount {
                        symbol: Some("BTC".to_string()),
                        mag: -50.0
                    },
                    "food"
                ))
            ))
        );

        // cost/balance assertions
        assert_eq!(
            parse_posting('.')("assets:checking 123.45 BTC @ 12345 ! 200.2 BTC"),
            Ok((
                "",
                Posting::Classic(ClassicPosting::new(
                    "assets:checking",
                    Some(Amount {
                        symbol: Some("BTC".to_string()),
                        mag: 123.45
                    }),
                    Some(Cost::UnitCost(Amount {
                        symbol: None,
                        mag: 12345.0,
                    })),
                    Some(Amount {
                        symbol: Some("BTC".to_string()),
                        mag: 200.2
                    }),
                ))
            ))
        );

        // ...in any order? with leftovers?
        assert_eq!(
            parse_posting('.')(
                "assets:checking 123.45 BTC ! 200.2 BTC @ 12345 ; a wild comment appeared!"
            ),
            Ok((
                " ; a wild comment appeared!",
                Posting::Classic(ClassicPosting::new(
                    "assets:checking",
                    Some(Amount {
                        symbol: Some("BTC".to_string()),
                        mag: 123.45
                    }),
                    Some(Cost::UnitCost(Amount {
                        symbol: None,
                        mag: 12345.0,
                    })),
                    Some(Amount {
                        symbol: Some("BTC".to_string()),
                        mag: 200.2
                    }),
                ))
            ))
        );

        // total cost assertions
        assert_eq!(
            parse_posting('.')(
                "expenses:yo 123.45 BTC == 100_000 ; double equals are nice"
            ),
            Ok((
                " ; double equals are nice",
                Posting::Classic(ClassicPosting::new(
                    "expenses:yo",
                    Some(Amount {
                        symbol: Some("BTC".to_string()),
                        mag: 123.45
                    }),
                    Some(Cost::TotalCost(Amount {
                        symbol: None,
                        mag: 100_000.0,
                    })),
                    None,
                ))
            ))
        );
    }
}
