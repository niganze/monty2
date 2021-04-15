use nom::sequence::tuple;
use nom::{branch::alt, IResult};

use crate::ast::{atom::Atom, Spanned};
use crate::parser::{token::PyToken, TokenSlice};

use super::{
    core::{expect_, expect_any_of},
    expect_with,
};

#[inline]
fn expect_digits<'a>(stream: TokenSlice<'a>) -> IResult<TokenSlice<'a>, Spanned<PyToken>> {
    expect_with(stream, |(tok, _)| matches!(tok, PyToken::Digits(_)))
}

#[inline]
pub fn expect_ident<'a>(stream: TokenSlice<'a>) -> IResult<TokenSlice<'a>, Spanned<PyToken>> {
    expect_with(stream, |(tok, _)| matches!(tok, PyToken::Ident(_)))
}

#[inline]
pub(crate) fn name<'a>(stream: TokenSlice<'a>) -> IResult<TokenSlice<'a>, Spanned<Atom>> {
    let (stream, ident) = expect_ident(stream)?;

    let name = if let PyToken::Ident(name) = ident.inner {
        Spanned {
            span: ident.span,
            inner: Atom::Name(name),
        }
    } else {
        unreachable!();
    };

    Ok((stream, name))
}

#[inline]
fn integer<'a>(stream: TokenSlice<'a>) -> IResult<TokenSlice<'a>, Spanned<Atom>> {
    let (stream, digits) = expect_digits(stream)?;

    let int = if let PyToken::Digits(inner) = digits.inner {
        Spanned {
            span: digits.span,
            inner: Atom::Int(inner),
        }
    } else {
        unreachable!();
    };

    Ok((stream, int))
}

#[inline]
fn float<'a>(stream: TokenSlice<'a>) -> IResult<TokenSlice<'a>, Spanned<Atom>> {
    let (stream, (left, _, right)) =
        tuple((expect_digits, expect_(PyToken::Dot), expect_digits))(stream)?;

    if let (PyToken::Digits(int), PyToken::Digits(real)) = (left.inner, right.inner) {
        // TODO(mental): figure out a way to parse floats more sensibly.
        let value = format!("{}.{}", int, real)
            .parse()
            .expect("Unable to parse float literal.");

        let float = Atom::Float(value);
        let span = left.span.start..right.span.end;

        Ok((stream, Spanned { span, inner: float }))
    } else {
        unreachable!()
    }
}

#[inline]
pub fn atom_unspanned<'a>(stream: TokenSlice<'a>) -> IResult<TokenSlice<'a>, Atom> {
    let (stream, Spanned { inner, .. }) = atom(stream)?;
    Ok((stream, inner))
}

#[inline]
pub fn atom<'a>(stream: TokenSlice<'a>) -> IResult<TokenSlice<'a>, Spanned<Atom>> {
    use PyToken::{Ellipsis, False, None, True};

    let fallback = |stream| {
        let (stream, Spanned { span, inner }) =
            expect_any_of([Ellipsis, True, False, None])(stream)?;

        let atom = Spanned {
            span,
            inner: Atom::from(inner),
        };

        Ok((stream, atom))
    };

    let (stream, atom) = alt((name, float, integer, fallback))(stream)?;

    Ok((stream, atom))
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::*;
    use crate::parser::{token::PyToken, Token};

    const PI: &[Token] = &[
        (PyToken::Digits(3), 0..1),
        (PyToken::Dot, 1..2),
        (PyToken::Digits(14), 2..4),
    ];

    #[test]
    fn test_parse_float() {
        let (stream, atom) = atom(PI).unwrap();
        assert!(stream.is_empty());
        assert!(
            matches!(atom, Spanned { span, inner } if (span == (0..4) && inner == Atom::Float(3.14)))
        );
    }

    const LIFE: &[Token] = &[(PyToken::Digits(42), 0..2)];

    #[test]
    fn test_parse_integer() {
        let (stream, atom) = atom(LIFE).unwrap();
        assert!(stream.is_empty());
        assert!(
            matches!(atom, Spanned { span, inner } if (span == (0..2) && inner == Atom::Int(42)))
        );
    }

    const NAME: &[Token] = &[(PyToken::Ident(NonZeroUsize::new(1)), 0..1)];

    #[test]
    fn test_parse_name() {
        let (stream, atom) = atom(NAME).unwrap();
        assert!(stream.is_empty());
        assert!(
            matches!(atom, Spanned { span, inner } if (span == (0..1) && inner == Atom::Name(NonZeroUsize::new(1))))
        );
    }
}