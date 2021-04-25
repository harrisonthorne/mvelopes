use nom::combinator::map_res;
use nom::sequence::preceded;
use nom::{IResult, branch::alt, bytes::complete::take_while1, character::complete::space0, combinator::map, sequence::separated_pair};

use crate::{amount::Amount, errors::ParseError};

use super::{is_amount_quantity_char, is_amount_symbol_char};

pub fn parse_amount(decimal_symbol: char) -> impl FnMut(&str) -> IResult<&str, Amount, ParseError> {
    move |input| {
        let amount_parser = alt((
            symbol_then_number(decimal_symbol),
            map(number_then_symbol(decimal_symbol), |(x, y)| (y, x)),
            map(number_only(decimal_symbol), |n| ("", n)),
        ));

        // using `preceded` in case the amount starts with whitespace
        map(preceded(space0, amount_parser), |(symbol_raw, quantity)| {
            Amount {
                symbol: if symbol_raw.trim().is_empty() {
                    // symbol_raw shouldn't have spaces in it, but better safe than sorry
                    None
                } else {
                    Some(symbol_raw.to_string())
                },
                mag: quantity,
            }
        })(&input)
        .map_err(|e| {
            e.map(|_| ParseError {
                context: Some(input.to_string()),
                message: Some(String::from("none of this could be parsed as an amount")),
            })
        })
    }
}

/// Returns (symbol, number)
fn symbol_then_number(decimal_symbol: char) -> impl FnMut(&str) -> IResult<&str, (&str, f64)> {
    move |input| separated_pair(symbol_only, space0, number_only(decimal_symbol))(input)
}

/// Returns (number, symbol)
fn number_then_symbol(decimal_symbol: char) -> impl FnMut(&str) -> IResult<&str, (f64, &str)> {
    move |input| separated_pair(number_only(decimal_symbol), space0, symbol_only)(input)
}

fn number_only(decimal_symbol: char) -> impl FnMut(&str) -> IResult<&str, f64> {
    move |input| {
        map_res(take_while1(is_amount_quantity_char), |x: &str| {
            // double negatives == positives so remove them
            let mut x = x.replace("--", "");

            // transform the quantity string into a format that Rust can parse.
            // remove thousands separators
            x = x
                .chars()
                .filter(|&c| c.is_digit(10) || c == '-' || c == decimal_symbol)
                .collect();

            // replace decimal symbol with dot, if needed
            if decimal_symbol != '.' {
                x = x.replace(decimal_symbol, ".")
            }

            x.parse::<f64>().map_err(|e| ParseError {
                context: Some(format!(r#""{}""#, input)),
                message: Some(format!(
                    "couldn't parse this as a number\nmore info: {:#?}",
                    e
                )),
            })
        })(input)
    }
}

fn symbol_only(input: &str) -> IResult<&str, &str> {
    take_while1(is_amount_symbol_char)(input)
}

mod tests {
    use super::*;

    #[test]
    fn test_symbol_then_number_separate() {
        assert_eq!(symbol_then_number('.')("$ 123"), Ok(("", ("$", 123.0))));
        assert_eq!(symbol_then_number('.')("Rs 123"), Ok(("", ("Rs", 123.0))));
        assert_eq!(symbol_then_number('.')("BTC 123"), Ok(("", ("BTC", 123.0))));
        assert_eq!(symbol_then_number(',')("p 123,92"), Ok(("", ("p", 123.92))));
        assert_eq!(symbol_then_number('.')("h 1 "), Ok((" ", ("h", 1.0))));
        assert_eq!(
            symbol_then_number('.')("$ 100 extra stuff"),
            Ok((" extra stuff", ("$", 100.0)))
        );
        assert!(symbol_then_number('.')(" h 1").is_err());
        assert!(symbol_then_number('.')("12").is_err());
        assert!(symbol_then_number('.')("$").is_err());
    }

    #[test]
    fn test_number_then_symbol_separate() {
        assert_eq!(number_then_symbol('.')("123 $"), Ok(("", (123., "$"))));
        assert_eq!(number_then_symbol('.')("123 Rs"), Ok(("", (123.0, "Rs"))));
        assert_eq!(number_then_symbol('.')("123 BTC"), Ok(("", (123.0, "BTC"))));
        assert_eq!(number_then_symbol(',')("123,92 p"), Ok(("", (123.92, "p"))));
        assert_eq!(number_then_symbol('.')("1 h "), Ok((" ", (1.0, "h"))));
        assert_eq!(
            number_then_symbol('.')("100 $ extra stuff"),
            Ok((" extra stuff", (100.0, "$")))
        );
        assert!(number_then_symbol('.')(" 1 h").is_err());
        assert!(number_then_symbol('.')("12").is_err());
        assert!(number_then_symbol('.')("$").is_err());
    }

    #[test]
    fn test_symbol_then_number_together() {
        assert_eq!(symbol_then_number('.')("$123"), Ok(("", ("$", 123.0))));
        assert_eq!(symbol_then_number('.')("Rs123"), Ok(("", ("Rs", 123.0))));
        assert_eq!(symbol_then_number('.')("BTC123"), Ok(("", ("BTC", 123.0))));
        assert_eq!(symbol_then_number(',')("p123,92"), Ok(("", ("p", 123.92))));
        assert_eq!(symbol_then_number('.')("h1 "), Ok((" ", ("h", 1.0))));
        assert_eq!(
            symbol_then_number('.')("$100 extra stuff"),
            Ok((" extra stuff", ("$", 100.0)))
        );
        assert!(symbol_then_number('.')(" h1").is_err());
        assert!(symbol_then_number('.')("12").is_err());
        assert!(symbol_then_number('.')("$").is_err());
    }

    #[test]
    fn test_number_then_symbol_together() {
        assert_eq!(number_then_symbol('.')("123$"), Ok(("", (123.0, "$"))));
        assert_eq!(number_then_symbol('.')("123Rs"), Ok(("", (123.0, "Rs"))));
        assert_eq!(number_then_symbol('.')("123BTC"), Ok(("", (123.0, "BTC"))));
        assert_eq!(number_then_symbol(',')("123,92p"), Ok(("", (123.92, "p"))));
        assert_eq!(number_then_symbol('.')("1h "), Ok((" ", (1.0, "h"))));
        assert_eq!(
            number_then_symbol('.')("100$ extra stuff"),
            Ok((" extra stuff", (100.0, "$")))
        );
        assert!(number_then_symbol('.')(" 1h").is_err());
        assert!(number_then_symbol('.')("12").is_err());
        assert!(number_then_symbol('.')("$").is_err());
    }

    #[test]
    fn test_number_only() {
        assert_eq!(number_only('.')("123"), Ok(("", 123.0)));
        assert_eq!(number_only('.')("456.789"), Ok(("", 456.789)));
        assert_eq!(number_only(',')("111.222.333,444"), Ok(("", 111_222_333.444)));
        assert_eq!(number_only('.')("111,222,333.444"), Ok(("", 111_222_333.444)));
        assert_eq!(number_only('.')("123BTC"), Ok(("BTC", 123.0)));
        assert_eq!(number_only('.')("123 BTC"), Ok((" BTC", 123.0)));
        assert!(number_only('.')(" 123").is_err());
        assert!(number_only('.')("$123").is_err());
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
            assert_eq!(parse_amount(dec)(input).unwrap(), expected);
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
