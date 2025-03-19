use crate::{
	lexing::{self, Delimiter, Keyword, Punct, Token, Tokens},
	BlankNode, Collection, Directive, Document, Iri, Lexer, Literal, Object, Objects,
	PredicateObjects, RdfLiteral, Statement, Subject, Triples, Verb,
};
use decoded_char::DecodedChar;
use locspan::Span;

/// Unexpected char or end of file.
#[derive(Debug, thiserror::Error)]
pub enum Unexpected {
	#[error("unexpected token `{0}`")]
	Token(Token),

	#[error("unexpected end of file")]
	EndOfFile,
}

impl From<Option<Token>> for Unexpected {
	fn from(value: Option<Token>) -> Self {
		match value {
			Some(token) => Unexpected::Token(token),
			None => Unexpected::EndOfFile,
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum Error<E> {
	#[error(transparent)]
	Lexer(E),

	#[error(transparent)]
	Unexpected(Unexpected),
}

pub type ParseError<E, M> = (Box<Error<E>>, M);

pub trait Parse<M>: Sized {
	#[allow(clippy::type_complexity)]
	fn parse_with<L, F>(parser: &mut Parser<L, F>) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		match parser.next()? {
			(Some(token), span) => Self::parse_from(parser, (token, span)),
			(None, span) => Self::parse_empty::<L>(parser.build_metadata(span)),
		}
	}

	#[allow(clippy::type_complexity)]
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		token: (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M;

	#[allow(clippy::type_complexity)]
	fn parse_empty<L>(meta: M) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
	{
		Err((Box::new(Error::Unexpected(Unexpected::EndOfFile)), meta))
	}

	#[inline(always)]
	fn parse<C, F, E>(
		chars: C,
		metadata_builder: F,
	) -> Result<(Self, M), ParseError<lexing::Error<E>, M>>
	where
		C: Iterator<Item = Result<DecodedChar, E>>,
		F: FnMut(Span) -> M,
	{
		let mut parser = Parser::new(Lexer::new(chars), metadata_builder);
		Self::parse_with(&mut parser)
	}

	#[inline(always)]
	fn parse_infallible<C, F>(
		chars: C,
		metadata_builder: F,
	) -> Result<(Self, M), ParseError<lexing::Error, M>>
	where
		C: Iterator<Item = DecodedChar>,
		F: FnMut(Span) -> M,
	{
		Self::parse(chars.map(Ok), metadata_builder)
	}

	#[inline(always)]
	fn parse_utf8<C, F, E>(
		chars: C,
		metadata_builder: F,
	) -> Result<(Self, M), ParseError<lexing::Error<E>, M>>
	where
		C: Iterator<Item = Result<char, E>>,
		F: FnMut(Span) -> M,
	{
		Self::parse(
			decoded_char::FallibleUtf8Decoded::new(chars),
			metadata_builder,
		)
	}

	#[inline(always)]
	fn parse_utf8_infallible<C, F>(
		chars: C,
		metadata_builder: F,
	) -> Result<(Self, M), ParseError<lexing::Error, M>>
	where
		C: Iterator<Item = char>,
		F: FnMut(Span) -> M,
	{
		Self::parse_infallible(decoded_char::Utf8Decoded::new(chars), metadata_builder)
	}

	#[inline(always)]
	fn parse_utf16<C, F, E>(
		chars: C,
		metadata_builder: F,
	) -> Result<(Self, M), ParseError<lexing::Error<E>, M>>
	where
		C: Iterator<Item = Result<char, E>>,
		F: FnMut(Span) -> M,
	{
		Self::parse(
			decoded_char::FallibleUtf16Decoded::new(chars),
			metadata_builder,
		)
	}

	#[inline(always)]
	fn parse_utf16_infallible<C, F>(
		chars: C,
		metadata_builder: F,
	) -> Result<(Self, M), ParseError<lexing::Error, M>>
	where
		C: Iterator<Item = char>,
		F: FnMut(Span) -> M,
	{
		Self::parse_infallible(decoded_char::Utf16Decoded::new(chars), metadata_builder)
	}

	#[inline(always)]
	fn parse_str<F>(
		string: &str,
		metadata_builder: F,
	) -> Result<(Self, M), ParseError<lexing::Error, M>>
	where
		F: FnMut(Span) -> M,
	{
		Self::parse_utf8_infallible(string.chars(), metadata_builder)
	}
}

pub struct Parser<L, F> {
	lexer: L,
	metadata_builder: F,
}

impl<L, F> Parser<L, F> {
	pub fn new(lexer: L, metadata_builder: F) -> Self {
		Self {
			lexer,
			metadata_builder,
		}
	}
}

impl<L: Tokens, F: FnMut(Span) -> M, M> Parser<L, F> {
	fn next(&mut self) -> Result<(Option<Token>, Span), ParseError<L::Error, M>> {
		self.lexer
			.next()
			.map_err(|(e, span)| (Box::new(Error::Lexer(e)), (self.metadata_builder)(span)))
	}

	#[allow(clippy::type_complexity)]
	fn peek(&mut self) -> Result<(Option<&Token>, Span), ParseError<L::Error, M>> {
		self.lexer
			.peek()
			.map_err(|(e, span)| (Box::new(Error::Lexer(e)), (self.metadata_builder)(span)))
	}

	fn last_span(&self) -> Span {
		self.lexer.last()
	}

	fn build_metadata(&mut self, span: Span) -> M {
		(self.metadata_builder)(span)
	}
}

impl<M> Parse<M> for Document<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, mut span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		let mut result = Document::default();
		result.insert(Statement::parse_from(parser, (token, span))?);

		while let (Some(token), span) = parser.next()? {
			result.insert(Statement::parse_from(parser, (token, span))?)
		}

		span.append(parser.last_span());
		Ok((result, parser.build_metadata(span)))
	}

	fn parse_empty<L>(meta: M) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
	{
		Ok((Self::new(), meta))
	}
}

impl<M> Parse<M> for Directive<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, mut span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		match token {
			Token::Keyword(Keyword::Prefix) => match parser.next()? {
				(Some(Token::CompactIri((namespace, ns_span), (suffix, suffix_span))), _) => {
					if suffix.is_empty() {
						match parser.next()? {
							(Some(Token::IriRef(iri_ref)), iri_ref_span) => match parser.next()? {
								(Some(Token::Punct(Punct::Period)), dot_span) => {
									span.append(dot_span);
									Ok((
										Directive::Prefix(
											(namespace, parser.build_metadata(ns_span)),
											(iri_ref, parser.build_metadata(iri_ref_span)),
										),
										parser.build_metadata(span),
									))
								}
								(unexpected, span) => Err((
									Box::new(Error::Unexpected(unexpected.into())),
									parser.build_metadata(span),
								)),
							},
							(unexpected, span) => Err((
								Box::new(Error::Unexpected(unexpected.into())),
								parser.build_metadata(span),
							)),
						}
					} else {
						Err((
							Box::new(Error::Unexpected(
								Some(Token::CompactIri(
									(namespace, ns_span),
									(suffix, suffix_span),
								))
								.into(),
							)),
							parser.build_metadata(span),
						))
					}
				}
				(unexpected, span) => Err((
					Box::new(Error::Unexpected(unexpected.into())),
					parser.build_metadata(span),
				)),
			},
			Token::Keyword(Keyword::Base) => match parser.next()? {
				(Some(Token::IriRef(iri_ref)), iri_ref_span) => match parser.next()? {
					(Some(Token::Punct(Punct::Period)), dot_span) => {
						span.append(dot_span);
						Ok((
							Directive::Base((iri_ref, parser.build_metadata(iri_ref_span))),
							parser.build_metadata(span),
						))
					}
					(unexpected, span) => Err((
						Box::new(Error::Unexpected(unexpected.into())),
						parser.build_metadata(span),
					)),
				},
				(unexpected, span) => Err((
					Box::new(Error::Unexpected(unexpected.into())),
					parser.build_metadata(span),
				)),
			},
			Token::Keyword(Keyword::SparqlPrefix) => match parser.next()? {
				(Some(Token::CompactIri((namespace, ns_span), (suffix, suffix_span))), _) => {
					if suffix.is_empty() {
						match parser.next()? {
							(Some(Token::IriRef(iri_ref)), iri_ref_span) => {
								span.append(iri_ref_span);
								Ok((
									Directive::SparqlPrefix(
										(namespace, parser.build_metadata(ns_span)),
										(iri_ref, parser.build_metadata(iri_ref_span)),
									),
									parser.build_metadata(span),
								))
							}
							(unexpected, span) => Err((
								Box::new(Error::Unexpected(unexpected.into())),
								parser.build_metadata(span),
							)),
						}
					} else {
						Err((
							Box::new(Error::Unexpected(
								Some(Token::CompactIri(
									(namespace, ns_span),
									(suffix, suffix_span),
								))
								.into(),
							)),
							parser.build_metadata(span),
						))
					}
				}
				(unexpected, span) => Err((
					Box::new(Error::Unexpected(unexpected.into())),
					parser.build_metadata(span),
				)),
			},
			Token::Keyword(Keyword::SparqlBase) => match parser.next()? {
				(Some(Token::IriRef(iri_ref)), iri_ref_span) => {
					span.append(iri_ref_span);
					Ok((
						Directive::SparqlBase((iri_ref, parser.build_metadata(iri_ref_span))),
						parser.build_metadata(span),
					))
				}
				(unexpected, span) => Err((
					Box::new(Error::Unexpected(unexpected.into())),
					parser.build_metadata(span),
				)),
			},
			unexpected => Err((
				Box::new(Error::Unexpected(Unexpected::Token(unexpected))),
				parser.build_metadata(span),
			)),
		}
	}
}

impl<M> Parse<M> for Statement<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		match token {
			token @ Token::Keyword(
				Keyword::Prefix | Keyword::Base | Keyword::SparqlPrefix | Keyword::SparqlBase,
			) => {
				let (directive, meta) = Directive::parse_from(parser, (token, span))?;
				Ok((Self::Directive(directive), meta))
			}
			token => {
				let (triples, meta) = Triples::parse_from(parser, (token, span))?;
				Ok((Self::Triples(triples), meta))
			}
		}
	}
}

impl<M> Parse<M> for Triples<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, mut span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		let subject = Subject::parse_from(parser, (token, span))?;

		let po_list = match parser.peek()? {
			(Some(Token::Punct(Punct::Period)), p_span) => {
				if !matches!(&subject, (Subject::BlankNode(BlankNode::Anonymous(l)), _) if !l.0.is_empty())
				{
					return Err((
						Box::new(Error::Unexpected(Unexpected::Token(Token::Punct(
							Punct::Period,
						)))),
						parser.build_metadata(p_span),
					));
				}

				let span = parser.last_span().next();
				(Vec::new(), parser.build_metadata(span))
			}
			_ => Vec::parse_with(parser)?,
		};

		span.append(parser.last_span());

		match parser.next()? {
			(Some(Token::Punct(Punct::Period)), _) => (),
			(unexpected, span) => {
				return Err((
					Box::new(Error::Unexpected(unexpected.into())),
					parser.build_metadata(span),
				));
			}
		}

		Ok((
			Triples {
				subject,
				predicate_objects_list: po_list,
			},
			parser.build_metadata(span),
		))
	}
}

impl<M> Parse<M> for Vec<(PredicateObjects<M>, M)> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		let mut result = vec![PredicateObjects::parse_from(parser, (token, span))?];

		loop {
			match parser.peek()? {
				(Some(Token::Punct(Punct::Semicolon)), _) => {
					parser.next()?;

					match parser.peek()? {
						(Some(Token::Punct(Punct::Period) | Token::End(Delimiter::Bracket)), _) => {
							break
						}
						_ => result.push(PredicateObjects::parse_with(parser)?),
					}
				}
				(Some(Token::Punct(Punct::Period) | Token::End(Delimiter::Bracket)), _) => break,
				_ => {
					let (unexpected, span) = parser.next()?;
					return Err((
						Box::new(Error::Unexpected(unexpected.into())),
						parser.build_metadata(span),
					));
				}
			}
		}

		Ok((result, parser.build_metadata(span)))
	}
}

impl<M> Parse<M> for PredicateObjects<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, mut span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		let verb = Verb::parse_from(parser, (token, span))?;
		let objects = Objects::parse_with(parser)?;
		span.append(parser.last_span());
		Ok((Self { verb, objects }, parser.build_metadata(span)))
	}
}

impl<M> Parse<M> for Objects<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		let mut result = vec![Object::parse_from(parser, (token, span))?];

		loop {
			match parser.peek()? {
				(Some(Token::Punct(Punct::Comma)), _) => {
					parser.next()?;
					result.push(Object::parse_with(parser)?);
				}
				(
					Some(
						Token::Punct(Punct::Period | Punct::Semicolon)
						| Token::End(Delimiter::Bracket),
					),
					_,
				) => break,
				_ => {
					let (unexpected, span) = parser.next()?;
					return Err((
						Box::new(Error::Unexpected(unexpected.into())),
						parser.build_metadata(span),
					));
				}
			}
		}

		Ok((Self(result), parser.build_metadata(span)))
	}
}

fn compact_iri<M, L, F>(
	parser: &mut Parser<L, F>,
	(prefix, prefix_span): (String, Span),
	(suffix, suffix_span): (String, Span),
) -> Iri<M>
where
	L: Tokens,
	F: FnMut(Span) -> M,
{
	Iri::PrefixedName(
		(prefix, parser.build_metadata(prefix_span)),
		(suffix, parser.build_metadata(suffix_span)),
	)
}

impl<M> Parse<M> for Subject<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, mut span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		match token {
			Token::IriRef(iri_ref) => Ok((
				Subject::Iri(Iri::IriRef(iri_ref)),
				parser.build_metadata(span),
			)),
			Token::CompactIri(prefix, suffix) => Ok((
				Subject::Iri(compact_iri(parser, prefix, suffix)),
				parser.build_metadata(span),
			)),
			Token::BlankNodeLabel(label) => Ok((
				Subject::BlankNode(BlankNode::Label(label)),
				parser.build_metadata(span),
			)),
			Token::Begin(Delimiter::Bracket) => {
				let po_list = match parser.peek()? {
					(Some(Token::End(Delimiter::Bracket)), _) => {
						let span = parser.last_span().next();
						(Vec::new(), parser.build_metadata(span))
					}
					_ => Vec::parse_with(parser)?,
				};

				match parser.next()? {
					(Some(Token::End(Delimiter::Bracket)), _) => (),
					(unexpected, span) => {
						return Err((
							Box::new(Error::Unexpected(unexpected.into())),
							parser.build_metadata(span),
						));
					}
				}

				span.append(parser.last_span());
				Ok((
					Subject::BlankNode(BlankNode::Anonymous(po_list)),
					parser.build_metadata(span),
				))
			}
			Token::Begin(Delimiter::Parenthesis) => {
				let (objects, meta) = Collection::parse_from(parser, (token, span))?;
				Ok((Subject::Collection(objects), meta))
			}
			unexpected => Err((
				Box::new(Error::Unexpected(Unexpected::Token(unexpected))),
				parser.build_metadata(span),
			)),
		}
	}
}

impl<M> Parse<M> for Collection<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, mut span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		match token {
			Token::Begin(Delimiter::Parenthesis) => {
				let mut objects = Vec::new();

				loop {
					match parser.next()? {
						(Some(Token::End(Delimiter::Parenthesis)), end_span) => {
							span.append(end_span);
							break;
						}
						(Some(token), span) => {
							let object = Object::parse_from(parser, (token, span))?;
							objects.push(object)
						}
						(unexpected, span) => {
							return Err((
								Box::new(Error::Unexpected(unexpected.into())),
								parser.build_metadata(span),
							))
						}
					}
				}

				Ok((Collection(objects), parser.build_metadata(span)))
			}
			unexpected => Err((
				Box::new(Error::Unexpected(Unexpected::Token(unexpected))),
				parser.build_metadata(span),
			)),
		}
	}
}

impl<M> Parse<M> for Object<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, mut span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		match token {
			Token::IriRef(iri_ref) => Ok((
				Object::Iri(Iri::IriRef(iri_ref)),
				parser.build_metadata(span),
			)),
			Token::CompactIri(prefix, suffix) => Ok((
				Object::Iri(compact_iri(parser, prefix, suffix)),
				parser.build_metadata(span),
			)),
			Token::BlankNodeLabel(label) => Ok((
				Object::BlankNode(BlankNode::Label(label)),
				parser.build_metadata(span),
			)),
			Token::Begin(Delimiter::Bracket) => {
				let po_list = match parser.peek()? {
					(Some(Token::End(Delimiter::Bracket)), _) => {
						let span = parser.last_span().next();
						(Vec::new(), parser.build_metadata(span))
					}
					_ => Vec::parse_with(parser)?,
				};

				match parser.next()? {
					(Some(Token::End(Delimiter::Bracket)), _) => (),
					(unexpected, span) => {
						return Err((
							Box::new(Error::Unexpected(unexpected.into())),
							parser.build_metadata(span),
						));
					}
				}

				span.append(parser.last_span());
				Ok((
					Object::BlankNode(BlankNode::Anonymous(po_list)),
					parser.build_metadata(span),
				))
			}
			Token::Begin(Delimiter::Parenthesis) => {
				let (objects, meta) = Collection::parse_from(parser, (token, span))?;
				Ok((Object::Collection(objects), meta))
			}
			token => {
				let (literal, meta) = Literal::parse_from(parser, (token, span))?;
				Ok((Object::Literal(literal), meta))
			}
		}
	}
}

const XSD_STRING: &iref::Iri = static_iref::iri!("http://www.w3.org/2001/XMLSchema#string");

#[allow(clippy::type_complexity)]
fn parse_rdf_literal<M, L, F>(
	parser: &mut Parser<L, F>,
	(string, string_span): (String, Span),
) -> Result<(RdfLiteral<M>, M), ParseError<L::Error, M>>
where
	L: Tokens,
	F: FnMut(Span) -> M,
{
	match parser.peek()? {
		(Some(Token::LangTag(_)), tag_span) => {
			let tag = match parser.next()? {
				(Some(Token::LangTag(tag)), _) => tag,
				_ => panic!("expected lang tag"),
			};

			let span = string_span.union(tag_span);
			Ok((
				RdfLiteral::new(
					(string, parser.build_metadata(string_span)),
					(
						rdf_types::LiteralType::LangString(tag),
						parser.build_metadata(tag_span),
					),
				),
				parser.build_metadata(span),
			))
		}
		(Some(Token::Punct(Punct::Carets)), _) => {
			parser.next()?;
			let (type_iri, metadata) = Iri::parse_with(parser)?;
			let literal_type = (rdf_types::LiteralType::Any(type_iri), metadata);
			let span = string_span.union(parser.last_span());
			Ok((
				RdfLiteral::new((string, parser.build_metadata(string_span)), literal_type),
				parser.build_metadata(span),
			))
		}
		_ => {
			let ty = (
				rdf_types::LiteralType::Any(Iri::IriRef(XSD_STRING.to_owned().into())),
				parser.build_metadata(string_span),
			);
			Ok((
				RdfLiteral::new((string, parser.build_metadata(string_span)), ty),
				parser.build_metadata(string_span),
			))
		}
	}
}

impl<M> Parse<M> for Literal<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		match token {
			Token::StringLiteral(string) => {
				let (lit, loc) = parse_rdf_literal(parser, (string, span))?;
				Ok((Literal::Rdf(lit), loc))
			}
			Token::Numeric(n) => Ok((Literal::Numeric(n), parser.build_metadata(span))),
			Token::Keyword(Keyword::True) => {
				Ok((Literal::Boolean(true), parser.build_metadata(span)))
			}
			Token::Keyword(Keyword::False) => {
				Ok((Literal::Boolean(false), parser.build_metadata(span)))
			}
			unexpected => Err((
				Box::new(Error::Unexpected(Unexpected::Token(unexpected))),
				parser.build_metadata(span),
			)),
		}
	}
}

impl<M> Parse<M> for Verb<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		match token {
			Token::Keyword(Keyword::A) => Ok((Verb::A, parser.build_metadata(span))),
			token => {
				let (iri, meta) = Iri::parse_from(parser, (token, span))?;
				Ok((Verb::Predicate(iri), meta))
			}
		}
	}
}

impl<M> Parse<M> for Iri<M> {
	fn parse_from<L, F>(
		parser: &mut Parser<L, F>,
		(token, span): (Token, Span),
	) -> Result<(Self, M), ParseError<L::Error, M>>
	where
		L: Tokens,
		F: FnMut(Span) -> M,
	{
		match token {
			Token::IriRef(iri_ref) => Ok((Iri::IriRef(iri_ref), parser.build_metadata(span))),
			Token::CompactIri(prefix, suffix) => Ok((
				compact_iri(parser, prefix, suffix),
				parser.build_metadata(span),
			)),
			unexpected => Err((
				Box::new(Error::Unexpected(Unexpected::Token(unexpected))),
				parser.build_metadata(span),
			)),
		}
	}
}
