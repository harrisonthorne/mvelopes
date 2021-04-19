use nom::sequence::preceded;
use nom::{
    branch::alt, bytes::complete::take_while1, character::complete::space0, combinator::map,
    sequence::separated_pair, IResult,
};

use crate::{amount::Amount, errors::ParseError};

use super::{is_amount_quantity_char, is_amount_symbol_char};

pub fn parse_amount(input: &str, decimal_symbol: char) -> IResult<&str, Amount, ParseError> {
    // reassign to remove double negatives
    let input = input.replace("--", "");

    let amount_parser = alt((
        symbol_then_number,
        map(number_then_symbol, |(x, y)| (y, x)),
        map(number_only, |n| ("", n)),
    ));

    // using `preceded` in case the amount starts with whitespace
    let (leftover, (symbol_raw, quantity_raw)) =
        preceded(space0, amount_parser)(&input).map_err(|e| e.map(|_| ParseError {
            context: Some(input.to_string()),
            message: Some(String::from("none of this could be parsed as an amount")),
        }))?;

    // transform the quantity string into a format that Rust can parse. replace `quantity_raw`
    let quantity_raw = {
        // remove thousands separators
        let x: String = quantity_raw
            .chars()
            .filter(|&c| c.is_digit(10) || c == '-' || c == decimal_symbol)
            .collect();

        // replace decimal symbol with dot, if needed
        if decimal_symbol != '.' {
            x.replace(decimal_symbol, ".")
        } else {
            x
        }
    };

    Ok((
        leftover,
        Amount {
            symbol: if symbol_raw.trim().is_empty() {
                // symbol_raw shouldn't have spaces in it, but better safe than sorry
                None
            } else {
                Some(symbol_raw.to_string())
            },
            mag: quantity_raw.parse().map_err(|e| nom::Err::Error(ParseError {
                context: Some(format!(r#""{}""#, input)),
                message: Some(format!(
                    "couldn't parse this as a number\nmore info: {:#?}",
                    e
                )),
            }))?,
        },
    ))
}

/// Returns (symbol, number)
fn symbol_then_number(input: &str) -> IResult<&str, (&str, &str)> {
    separated_pair(symbol_only, space0, number_only)(input)
}

/// Returns (number, symbol)
fn number_then_symbol(input: &str) -> IResult<&str, (&str, &str)> {
    separated_pair(number_only, space0, symbol_only)(input)
}

fn number_only(input: &str) -> IResult<&str, &str> {
    take_while1(is_amount_quantity_char)(input)
}

fn symbol_only(input: &str) -> IResult<&str, &str> {
    take_while1(is_amount_symbol_char)(input)
}

mod tests {
    use super::*;

    #[test]
    fn test_symbol_then_number_separate() {
        assert_eq!(symbol_then_number("$ 123"), Ok(("", ("$", "123"))));
        assert_eq!(symbol_then_number("Rs 123"), Ok(("", ("Rs", "123"))));
        assert_eq!(symbol_then_number("BTC 123"), Ok(("", ("BTC", "123"))));
        assert_eq!(symbol_then_number("p 123,92"), Ok(("", ("p", "123,92"))));
        assert_eq!(symbol_then_number("h 1 "), Ok((" ", ("h", "1"))));
        assert_eq!(
            symbol_then_number("$ 100 extra stuff"),
            Ok((" extra stuff", ("$", "100")))
        );
        assert!(symbol_then_number(" h 1").is_err());
        assert!(symbol_then_number("12").is_err());
        assert!(symbol_then_number("$").is_err());
    }

    #[test]
    fn test_number_then_symbol_separate() {
        assert_eq!(number_then_symbol("123 $"), Ok(("", ("123", "$"))));
        assert_eq!(number_then_symbol("123 Rs"), Ok(("", ("123", "Rs"))));
        assert_eq!(number_then_symbol("123 BTC"), Ok(("", ("123", "BTC"))));
        assert_eq!(number_then_symbol("123,92 p"), Ok(("", ("123,92", "p"))));
        assert_eq!(number_then_symbol("1 h "), Ok((" ", ("1", "h"))));
        assert_eq!(
            number_then_symbol("100 $ extra stuff"),
            Ok((" extra stuff", ("100", "$")))
        );
        assert!(number_then_symbol(" 1 h").is_err());
        assert!(number_then_symbol("12").is_err());
        assert!(number_then_symbol("$").is_err());
    }

    #[test]
    fn test_symbol_then_number_together() {
        assert_eq!(symbol_then_number("$123"), Ok(("", ("$", "123"))));
        assert_eq!(symbol_then_number("Rs123"), Ok(("", ("Rs", "123"))));
        assert_eq!(symbol_then_number("BTC123"), Ok(("", ("BTC", "123"))));
        assert_eq!(symbol_then_number("p123,92"), Ok(("", ("p", "123,92"))));
        assert_eq!(symbol_then_number("h1 "), Ok((" ", ("h", "1"))));
        assert_eq!(
            symbol_then_number("$100 extra stuff"),
            Ok((" extra stuff", ("$", "100")))
        );
        assert!(symbol_then_number(" h1").is_err());
        assert!(symbol_then_number("12").is_err());
        assert!(symbol_then_number("$").is_err());
    }

    #[test]
    fn test_number_then_symbol_together() {
        assert_eq!(number_then_symbol("123$"), Ok(("", ("123", "$"))));
        assert_eq!(number_then_symbol("123Rs"), Ok(("", ("123", "Rs"))));
        assert_eq!(number_then_symbol("123BTC"), Ok(("", ("123", "BTC"))));
        assert_eq!(number_then_symbol("123,92p"), Ok(("", ("123,92", "p"))));
        assert_eq!(number_then_symbol("1h "), Ok((" ", ("1", "h"))));
        assert_eq!(
            number_then_symbol("100$ extra stuff"),
            Ok((" extra stuff", ("100", "$")))
        );
        assert!(number_then_symbol(" 1h").is_err());
        assert!(number_then_symbol("12").is_err());
        assert!(number_then_symbol("$").is_err());
    }

    #[test]
    fn test_number_only() {
        assert_eq!(number_only("123"), Ok(("", "123")));
        assert_eq!(number_only("456.789"), Ok(("", "456.789")));
        assert_eq!(number_only("111.222.333,444"), Ok(("", "111.222.333,444")));
        assert_eq!(number_only("111,222,333.444"), Ok(("", "111,222,333.444")));
        assert_eq!(number_only("123BTC"), Ok(("BTC", "123")));
        assert_eq!(number_only("123 BTC"), Ok((" BTC", "123")));
        assert!(number_only(" 123").is_err());
        assert!(number_only("$123").is_err());
    }

    #[test]
    fn test_symbol_only() {
        assert_eq!(symbol_only("$"), Ok(("", "$")));
        assert_eq!(symbol_only("$100"), Ok(("100", "$")));
        assert_eq!(symbol_only("$.10"), Ok((".10", "$")));
        assert_eq!(symbol_only("BTC123"), Ok(("123", "BTC")));
        assert_eq!(symbol_only("BTC 123"), Ok((" 123", "BTC")));
        assert!(symbol_only(" $").is_err());
        assert!(symbol_only("100$").is_err());
        assert!(symbol_only(" 100$").is_err());
    }

    #[test]
    fn test_parse_amount() {
        let amount = |symbol, quant| Amount {
            symbol: Some(String::from(symbol)),
            mag: quant,
        };
        let test = |input, dec, expected| {
            assert_eq!(parse_amount(input, dec).unwrap(), expected);
        };

        test("$100", '.', ("", amount("$", 100.0)));
        test("12.34 BTC", '.', ("", amount("BTC", 12.34)));
        test("56.78Y", '.', ("", amount("Y", 56.78)));
        test("pts 910.11", '.', ("", amount("pts", 910.11)));
        test("%20.", '.', ("", amount("%", 20.0)));
        test("$100.000,4", ',', ("", amount("$", 100_000.4)));
        test("$,6", ',', ("", amount("$", 0.6)));
        test(
            "$1_000_000.5",
            '.',
            ("", amount("$", 1_000_000.5)),
        );
        test(
            "$1_000_000,123_456",
            ',',
            ("", amount("$", 1_000_000.123456)),
        );

        test(
            "$123 ; a wild comment appeared!",
            '.',
            (
                " ; a wild comment appeared!",
                amount("$", 123.0),
            ),
        );
        test(
            "127h//yoink",
            '.',
            ("//yoink", amount("h", 127.0)),
        );

        test("$100 ex", '.', (" ex", amount("$", 100.0)));
        test(
            "BTC100.oops",
            '.',
            ("oops", amount("BTC", 100.0)),
        );
        test(
            "500 ETH weiner",
            '.',
            (" weiner", amount("ETH", 500.0)),
        );
        test(
            "456.7 DOGE boye",
            '.',
            (" boye", amount("DOGE", 456.7)),
        );
        test(
            "891,1 commas extra",
            ',',
            (" extra", amount("commas", 891.1)),
        );

        // testing leading spaces
        test(" 600spaces", '.', ("", amount("spaces", 600.0)));
        test(
            "\t2_000.watts",
            '.',
            ("", amount("watts", 2000.0)),
        );
    }
}
