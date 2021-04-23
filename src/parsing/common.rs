use nom::multi::many1;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::space0,
    combinator::map,
    sequence::pair,
    sequence::preceded,
    IResult,
};

#[allow(clippy::result_unit_err)]
pub fn eol_comment(line: &str) -> IResult<&str, &str> {
    let comment_start = alt((tag("//"), tag(";")));
    map(
        pair(comment_start, preceded(space0, is_not("\r\n"))),
        |(_, y)| y,
    )(line)
}

/// Returns true if the char is a digit, period, comma, or dash.
pub fn is_amount_quantity_char(c: char) -> bool {
    c.is_digit(10) || c == ',' || c == '.' || c == '-'
}

/// Returns true if the char can quality as the symbol for an amount.
pub fn is_amount_symbol_char(c: char) -> bool {
    !is_amount_quantity_char(c) && !c.is_whitespace()
}

mod tests {
    use super::*;

    #[test]
    fn test_eol_comment() {
        assert_eq!(eol_comment("// this is a slash comment"), Ok(("", "this is a slash comment")));
        assert_eq!(eol_comment("; this is a semicolon comment"), Ok(("", "this is a semicolon comment")));

        // we've opted to preserve extra comment symbols
        assert_eq!(eol_comment("//// thicc comment"), Ok(("", "// thicc comment")));
        assert_eq!(eol_comment(";;;; also thicc comment"), Ok(("", ";;; also thicc comment")));

        assert_eq!(eol_comment("//nice and comfortable"), Ok(("", "nice and comfortable")));
        assert_eq!(eol_comment(";cozy"), Ok(("", "cozy")));

        assert_eq!(eol_comment("///thicc"), Ok(("", "/thicc")));
        assert_eq!(eol_comment(";;;boi"), Ok(("", ";;boi")));

        assert!(eol_comment(" // won't parse if spaces come before").is_err());
        assert!(eol_comment("/ two slashes are needed").is_err());
        assert!(eol_comment("\t; tabs aren't allowed at the beginning either").is_err());
    }
}
