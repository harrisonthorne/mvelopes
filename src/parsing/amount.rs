use nom::bytes::complete::take_while1;
use nom::character::complete::space1;
use nom::combinator::map;
use nom::sequence::pair;
use nom::sequence::separated_pair;
use nom::{IResult, branch::alt, number::complete::f32 as nom_f32};

use crate::amount::Amount;
use crate::errors::ParseError;
use crate::errors::SilverfoxResult;

use super::is_amount_quantity_char;
use super::is_amount_symbol_char;

// pub fn parse_amount(input: &str) -> SilverfoxResult<Amount> {
//     let (leftover, (symbol_raw, quantity_raw)) = {
//         alt((
//             symbol_then_number_together,
//             map(number_then_symbol_separate, |(x, y)| (y, x)),
//             symbol_then_number_separate,
//             map(number_then_symbol_together, |(x, y)| (y, x)),
//             number_only,
//         ))
//     }(input).map_err(|_| ParseError {
//         context: Some(input.to_string()),
//         message: Some(String::from("none of this could be parsed as an amount")),

//     })?;

//     Ok(Amount {
//         symbol: if symbol_raw.trim().is_empty() {
//             None
//         } else {
//             Some(symbol_raw.to_string())
//         },
//         mag: quantity_raw.parse().map_err(|e| crate::errors::SilverfoxError::Parse(ParseError {
//             context: Some(format!(r#""{}""#, input)),
//             message: Some(String::from("couldn't parse this as a number")),
//         }))?,
//     })
// }

fn symbol_then_number_separate(input: &str) -> IResult<&str, (&str, &str)> {
    separated_pair(symbol_only, space1, number_only)(input)
}

fn number_then_symbol_separate(input: &str) -> IResult<&str, (&str, &str)> {
    separated_pair(number_only, space1, symbol_only)(input)
}

fn symbol_then_number_together(input: &str) -> IResult<&str, (&str, &str)> {
    pair(symbol_only, number_only)(input)
}

fn number_then_symbol_together(input: &str) -> IResult<&str, (&str, &str)> {
    pair(number_only, symbol_only)(input)
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
        assert_eq!(symbol_then_number_separate("$ 123"), Ok(("", ("$", "123"))));
        assert_eq!(symbol_then_number_separate("Rs 123"), Ok(("", ("Rs", "123"))));
        assert_eq!(symbol_then_number_separate("BTC 123"), Ok(("", ("BTC", "123"))));
        assert_eq!(symbol_then_number_separate("p 123,92"), Ok(("", ("p", "123,92"))));
        assert_eq!(symbol_then_number_separate("h 1 "), Ok((" ", ("h", "1"))));
        assert_eq!(symbol_then_number_separate("$ 100 extra stuff"), Ok((" extra stuff", ("$", "100"))));
        assert!(symbol_then_number_separate(" h 1").is_err());
        assert!(symbol_then_number_separate("12").is_err());
        assert!(symbol_then_number_separate("$").is_err());
    }

    #[test]
    fn test_number_then_symbol_separate() {
        assert_eq!(number_then_symbol_separate("123 $"), Ok(("", ("123", "$"))));
        assert_eq!(number_then_symbol_separate("123 Rs"), Ok(("", ("123", "Rs"))));
        assert_eq!(number_then_symbol_separate("123 BTC"), Ok(("", ("123", "BTC"))));
        assert_eq!(number_then_symbol_separate("123,92 p"), Ok(("", ("123,92", "p"))));
        assert_eq!(number_then_symbol_separate("1 h "), Ok((" ", ("1", "h"))));
        assert_eq!(number_then_symbol_separate("100 $ extra stuff"), Ok((" extra stuff", ("100", "$"))));
        assert!(number_then_symbol_separate(" 1 h").is_err());
        assert!(number_then_symbol_separate("12").is_err());
        assert!(number_then_symbol_separate("$").is_err());
    }

    #[test]
    fn test_symbol_then_number_together() {
        assert_eq!(symbol_then_number_together("$123"), Ok(("", ("$", "123"))));
        assert_eq!(symbol_then_number_together("Rs123"), Ok(("", ("Rs", "123"))));
        assert_eq!(symbol_then_number_together("BTC123"), Ok(("", ("BTC", "123"))));
        assert_eq!(symbol_then_number_together("p123,92"), Ok(("", ("p", "123,92"))));
        assert_eq!(symbol_then_number_together("h1 "), Ok((" ", ("h", "1"))));
        assert_eq!(symbol_then_number_together("$100 extra stuff"), Ok((" extra stuff", ("$", "100"))));
        assert!(symbol_then_number_together(" h1").is_err());
        assert!(symbol_then_number_together("12").is_err());
        assert!(symbol_then_number_together("$").is_err());
    }

    #[test]
    fn test_number_then_symbol_together() {
        assert_eq!(number_then_symbol_together("123$"), Ok(("", ("123", "$"))));
        assert_eq!(number_then_symbol_together("123Rs"), Ok(("", ("123", "Rs"))));
        assert_eq!(number_then_symbol_together("123BTC"), Ok(("", ("123", "BTC"))));
        assert_eq!(number_then_symbol_together("123,92p"), Ok(("", ("123,92", "p"))));
        assert_eq!(number_then_symbol_together("1h "), Ok((" ", ("1", "h"))));
        assert_eq!(number_then_symbol_together("100$ extra stuff"), Ok((" extra stuff", ("100", "$"))));
        assert!(number_then_symbol_together(" 1h").is_err());
        assert!(number_then_symbol_together("12").is_err());
        assert!(number_then_symbol_together("$").is_err());
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
}
