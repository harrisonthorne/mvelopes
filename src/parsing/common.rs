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
