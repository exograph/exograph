use nom::character::complete::{one_of, space0};
use nom::combinator::recognize;
use nom::sequence::preceded;
use nom::{
    branch::alt,
    character::complete::multispace0,
    sequence::{pair, terminated},
};
use nom::{bytes::complete::escaped, combinator::map};
use nom::{
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, char},
};
use nom::{combinator::cut, sequence::delimited};
use nom::{error::context, multi::many0};

use super::PResult;

pub fn ws<'a, F: 'a, O>(inner: F) -> impl FnMut(&'a str) -> PResult<&'a str, O>
where
    F: FnMut(&'a str) -> PResult<&'a str, O>,
{
    delimited(multispace0, inner, multispace0)
}

pub fn spaces<'a, F: 'a, O>(inner: F) -> impl FnMut(&'a str) -> PResult<&'a str, O>
where
    F: FnMut(&'a str) -> PResult<&'a str, O>,
{
    delimited(space0, inner, space0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Identifier<'a>(pub &'a str);

pub fn identifier<'a>(input: &'a str) -> PResult<&'a str, Identifier<'a>> {
    map(
        recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_")))),
        )),
        |ident_str| Identifier(ident_str),
    )(input)
}

pub fn quoted_identifier<'a>(input: &'a str) -> PResult<&'a str, Identifier<'a>> {
    delimited(char('"'), identifier, char('"'))(input)
}

fn parse_str<'a>(input: &'a str) -> PResult<&'a str, &'a str> {
    escaped(alphanumeric1, '\\', one_of("\"n\\"))(input)
}

pub fn string<'a>(input: &'a str) -> PResult<&'a str, &'a str> {
    context(
        "string",
        preceded(char('\"'), cut(terminated(parse_str, char('\"')))),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_identifier() {
        assert_eq!(identifier("id"), Ok(("", Identifier("id"))));
        assert_eq!(identifier("team_id"), Ok(("", Identifier("team_id"))));
    }
}
