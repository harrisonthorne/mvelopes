use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    combinator::value,
    sequence::pair,
    IResult,
};

#[allow(clippy::result_unit_err)]
pub fn eol_comment(line: &str) -> IResult<&str, ()> {
    let comment_start = alt((tag("//"), tag(";")));
    value((), pair(comment_start, is_not("\r\n")))(line)
}

/// Returns true if the char is a digit, period, comma, or dash.
pub fn is_amount_quantity_char(c: char) -> bool {
    c.is_digit(10) || c == ',' || c == '.' || c == '-'
}

/// Returns true if the char can quality as the symbol for an amount.
pub fn is_amount_symbol_char(c: char) -> bool {
    !is_amount_quantity_char(c) && !c.is_whitespace()
}

// mod tests {
//     use super::*;

//     #[test]
//     fn test_eol_comment() {
//         assert_eq!(eol_comment("testing comments ; this is a comment"), Ok(("testing comments ", ())));
//     }
// }
