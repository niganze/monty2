use nom::{IResult, Err, error::{Error, ErrorKind}, multi::{many0, many1, many_m_n}};

use crate::ast::Spanned;
use crate::parser::{Span, TokenSlice, token::PyToken};


#[inline]
pub fn expect_many_n<const N: usize>(
    value: PyToken,
) -> impl for<'a> Fn(TokenSlice<'a>) -> IResult<TokenSlice<'a>, Vec<Spanned<PyToken>>> {
    move |stream| match N {
        0 => many0(expect_(value))(stream),
        1 => many1(expect_(value))(stream),
        m => many_m_n(m, m.saturating_add(1), expect_(value))(stream),
    }
}

#[inline]
pub fn expect_(
    value: PyToken,
) -> impl for<'a> Fn(TokenSlice<'a>) -> IResult<TokenSlice<'a>, Spanned<PyToken>> {
    move |stream: TokenSlice<'_>| {
        let (stream, result) = expect(stream, value)?;
        Ok((stream, result))
    }
}

// #[inline]
// pub fn expect_token(
//     value: PyToken,
// ) -> impl for<'a> Fn(TokenSlice<'a>) -> IResult<TokenSlice<'a>, (PyToken, AstObject)> {
//     move |stream: TokenSlice<'_>| expect(stream, value)
// }

#[inline]
pub fn expect_any_of<const N: usize>(
    values: [PyToken; N],
) -> impl for<'a> Fn(TokenSlice<'a>) -> IResult<TokenSlice<'a>, Spanned<PyToken>> {
    move |stream: TokenSlice<'_>| {
        for token in values.iter() {
            match expect(stream, *token) {
                Ok(r) => return Ok(r),
                Err(_) => continue,
            }
        }

        Err(Err::Error(Error::new(stream, ErrorKind::Alt)))
    }
}

#[inline]
pub fn expect_any_token<const N: usize>(
    values: [PyToken; N],
) -> impl for<'a> Fn(TokenSlice<'a>) -> IResult<TokenSlice<'a>, Spanned<PyToken>> {
    move |stream: TokenSlice<'_>| {
        for token in values.iter() {
            match expect(stream, *token) {
                Ok(r) => return Ok(r),
                Err(_) => continue,
            }
        }

        Err(Err::Error(Error::new(stream, ErrorKind::Alt)))
    }
}

#[inline]
pub fn expect_with<'a, F>(
    stream: TokenSlice<'a>,
    predicate: F,
) -> IResult<TokenSlice<'a>, Spanned<PyToken>>
where
    F: Fn((&PyToken, &Span)) -> bool,
{
    match stream {
        [(token, span), rest @ ..] => {
            if predicate((token, span)) {
                let (token, span) = (token.clone(), span.clone());
                Ok((rest, Spanned { span, inner: token }))
            } else {
                let err = Error::new(stream, ErrorKind::IsNot);
                Err(Err::Error(err))
            }
        }

        [] => Err(Err::Error(Error::new(stream, ErrorKind::Eof))),
    }
}

#[inline]
pub fn expect<'a>(
    stream: TokenSlice<'a>,
    value: PyToken,
) -> IResult<TokenSlice<'a>, Spanned<PyToken>> {
    expect_with(stream, move |(tok, _)| *tok == value)
}

pub fn expect_wrapped_values<const N: usize>(
    values: [PyToken; N],
    wrapper: PyToken,
) -> impl for<'a> Fn(TokenSlice<'a>) -> IResult<TokenSlice<'a>, Vec<Spanned<PyToken>>> {
    if N == 0 {
        panic!("Must proved a value array that contains at least one token, not zero.")
    }

    move |stream| {
        let (mut stream, _) = expect_many_n::<0>(wrapper)(stream)?;

        let mut results = Vec::with_capacity(N);

        for token in values.iter() {
            let (s, obj) = expect(stream, *token)?;
            stream = s;
            results.push(obj)
        }

        let (stream, _) = expect_many_n::<0>(wrapper)(stream)?;

        Ok((stream, results))
    }
}


// #[inline]
// pub fn parse_single_binop<'a, F>(
//     stream: TokenSlice<'a>,
//     parser: fn(TokenSlice<'a>) -> IResult<TokenSlice<'a>, AstObject>,
//     ctor: F,
//     tok: PyToken,
// ) -> IResult<TokenSlice<'a>, AstObject>
// where
//     F: FnOnce(Box<AstObject>, Box<AstObject>) -> AstNode,
// {
//     let (stream, _) = expect_many_n::<0>(PyToken::Whitespace)(stream)?;
//     let (stream, base) = parser(stream)?;

//     if let Ok((stream, _)) = expect(stream, tok) {
//         let (stream, _) = expect_many_n::<0>(PyToken::Whitespace)(stream)?;
//         let (stream, value) = parser(stream)?;

//         let span = base.span.start..value.span.end;

//         let left = Box::new(base);
//         let right = Box::new(value);

//         let obj = AstObject {
//             span,
//             inner: ctor(left, right),
//         };

//         Ok((stream, obj))
//     } else {
//         Ok((stream, base))
//     }
// }

// #[inline]
// pub fn parse_unary<'a, P, F>(
//     stream: TokenSlice<'a>,
//     prefix: P,
//     parser: fn(TokenSlice<'a>) -> IResult<TokenSlice<'a>, AstObject>,
//     ctor: F,
// ) -> IResult<TokenSlice<'a>, AstObject>
// where
//     P: FnOnce(TokenSlice<'a>) -> IResult<TokenSlice<'a>, AstObject>,
//     F: FnOnce(Box<AstObject>) -> AstNode,
// {
//     let (stream, first) = prefix(stream)?;
//     let (stream, value) = parser(stream)?;

//     let span = first.span.start..value.span.end;
//     let inner = ctor(Box::new(value));

//     let obj = AstObject { span, inner };

//     Ok((stream, obj))
// }
