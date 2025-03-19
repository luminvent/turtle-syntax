use iref::{Iri, IriBuf, IriRef, IriRefBuf};
use langtag::LangTagBuf;
use rdf_types::vocabulary::{
	self, BlankIdVocabulary, BlankIdVocabularyMut, IriVocabulary, IriVocabularyMut,
};
use rdf_types::{Generator, Id, LexicalTriple, LiteralType, Object, Subject, Term, Triple};
use static_iref::iri;
use std::collections::HashMap;

const RDF_TYPE: &Iri = iri!("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
const RDF_LIST: &Iri = iri!("http://www.w3.org/1999/02/22-rdf-syntax-ns#List");
const RDF_NIL: &Iri = iri!("http://www.w3.org/1999/02/22-rdf-syntax-ns#nil");
const RDF_FIRST: &Iri = iri!("http://www.w3.org/1999/02/22-rdf-syntax-ns#first");
const RDF_REST: &Iri = iri!("http://www.w3.org/1999/02/22-rdf-syntax-ns#rest");
const XSD_BOOLEAN: &Iri = iri!("http://www.w3.org/2001/XMLSchema#boolean");
const XSD_INTEGER: &Iri = iri!("http://www.w3.org/2001/XMLSchema#integer");
const XSD_DECIMAL: &Iri = iri!("http://www.w3.org/2001/XMLSchema#decimal");
const XSD_DOUBLE: &Iri = iri!("http://www.w3.org/2001/XMLSchema#double");

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("cannot resolve relative IRI <{0}>: no base IRI")]
	NoBaseIri(IriRefBuf),

	#[error("unknown IRI prefix `{0}`")]
	UnknownPrefix(String),

	#[error("invalid compact IRI suffix in `{prefix}:{invalid_suffix}`")]
	InvalidCompactIriSuffix {
		prefix: String,
		iri: IriBuf,
		invalid_suffix: String,
	},
}

pub type BuildError<M> = (Box<Error>, M);

pub type RdfId<V> = Id<<V as IriVocabulary>::Iri, <V as BlankIdVocabulary>::BlankId>;

pub type MetaTriple<M, V = ()> = (
	Triple<
		(RdfId<V>, M),
		(<V as IriVocabulary>::Iri, M),
		(
			Object<RdfId<V>, crate::RdfLiteral<M, <V as IriVocabulary>::Iri>>,
			M,
		),
	>,
	M,
);

impl<M: Clone> crate::Document<M> {
	pub fn build_meta_triples(
		&self,
		base_iri: Option<IriBuf>,
		mut generator: impl Generator<()>,
	) -> Result<Vec<MetaTriple<M, ()>>, BuildError<M>> {
		let mut triples = Vec::new();
		let mut context = Context::new(base_iri, vocabulary::no_vocabulary_mut(), &mut generator);
		self.build(&mut context, &mut triples)?;
		Ok(triples)
	}

	pub fn build_meta_triples_with<V: RdfVocabulary + IriVocabularyMut + BlankIdVocabularyMut>(
		&self,
		base_iri: Option<V::Iri>,
		vocabulary: &mut V,
		mut generator: impl Generator<V>,
	) -> Result<Vec<MetaTriple<M, V>>, BuildError<M>>
	where
		V::Iri: Clone,
		V::BlankId: Clone,
	{
		let mut triples = Vec::new();
		let mut context = Context::new(base_iri, vocabulary, &mut generator);
		self.build(&mut context, &mut triples)?;
		Ok(triples)
	}

	pub fn build_lexical_triples(
		&self,
		base_iri: Option<IriBuf>,
		mut generator: impl Generator<()>,
	) -> Result<Vec<LexicalTriple>, BuildError<M>> {
		let triples = self.build_meta_triples(base_iri, &mut generator)?;

		let triples = triples.into_iter().map(|triple| {
			let o = match triple.0 .2 .0 {
				Term::Id(id) => Term::Id::<rdf_types::Id, rdf_types::Literal>(id),
				Term::Literal(literal) => Term::Literal(literal.into()),
			};

			LexicalTriple::new(triple.0 .0 .0, triple.0 .1 .0, o)
		}).collect();

		Ok(triples)
	}
}

pub struct Context<'v, 'g, M, V: IriVocabulary, G> {
	vocabulary: &'v mut V,
	generator: &'g mut G,
	base_iri: Option<V::Iri>,
	prefixes: HashMap<String, (V::Iri, M)>,
}

impl<'v, 'g, M, V: IriVocabulary, G> Context<'v, 'g, M, V, G> {
	pub fn new(base_iri: Option<V::Iri>, vocabulary: &'v mut V, generator: &'g mut G) -> Self {
		Self {
			vocabulary,
			generator,
			base_iri,
			prefixes: HashMap::new(),
		}
	}

	pub fn resolve_iri_ref(
		&mut self,
		(iri_ref, meta): (&IriRef, &M),
	) -> Result<V::Iri, BuildError<M>>
	where
		M: Clone,
		V: IriVocabularyMut,
	{
		match &self.base_iri {
			Some(current) => {
				let iri = iri_ref.resolved(self.vocabulary.iri(current).unwrap());
				Ok(self.vocabulary.insert(iri.as_iri()))
			}
			None => match iri_ref.as_iri() {
				Some(iri) => Ok(self.vocabulary.insert(iri)),
				None => Err((Box::new(Error::NoBaseIri(iri_ref.to_owned())), meta.clone())),
			},
		}
	}

	pub fn resolve_compact_iri(
		&mut self,
		prefix: (&str, &M),
		suffix: (&str, &M),
		meta: &M,
	) -> Result<V::Iri, BuildError<M>>
	where
		M: Clone,
		V: IriVocabularyMut,
	{
		match self.prefixes.get(prefix.0) {
			Some(iri) => {
				let iri = self.vocabulary.iri(&iri.0).unwrap();
				let mut buffer = iri.to_string();
				buffer.push_str(suffix.0);
				match Iri::new(&buffer) {
					Ok(result) => Ok(self.vocabulary.insert(result)),
					Err(_) => Err((
						Box::new(Error::InvalidCompactIriSuffix {
							prefix: prefix.0.to_owned(),
							iri: iri.to_owned(),
							invalid_suffix: suffix.0.to_owned(),
						}),
						meta.clone(),
					)),
				}
			}
			None => Err((
				Box::new(Error::UnknownPrefix(prefix.0.to_owned())),
				prefix.1.clone(),
			)),
		}
	}

	pub fn insert_prefix(&mut self, prefix: String, iri: V::Iri, meta: M) {
		self.prefixes.insert(prefix, (iri, meta));
	}
}

pub trait RdfVocabulary: IriVocabulary + BlankIdVocabulary {}

impl<V> RdfVocabulary for V where V: IriVocabulary + BlankIdVocabulary {}

pub trait Build<M, V: RdfVocabulary, G> {
	fn build(
		&self,
		context: &mut Context<M, V, G>,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<(), BuildError<M>>;
}

impl<M: Clone, V: RdfVocabulary + IriVocabularyMut + BlankIdVocabularyMut, G: Generator<V>>
	Build<M, V, G> for crate::Document<M>
where
	V::Iri: Clone,
	V::BlankId: Clone,
{
	fn build(
		&self,
		context: &mut Context<M, V, G>,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<(), BuildError<M>> {
		for statement in &self.statements {
			match statement {
				(crate::Statement::Directive(directive), meta) => match directive {
					crate::Directive::Base(iri) | crate::Directive::SparqlBase(iri) => {
						let iri_ref = (iri.0.as_iri_ref(), &iri.1);
						context.base_iri = Some(context.resolve_iri_ref(iri_ref)?);
					}
					crate::Directive::Prefix(prefix, iri)
					| crate::Directive::SparqlPrefix(prefix, iri) => {
						let iri_ref = (iri.0.as_iri_ref(), &iri.1);
						let iri = context.resolve_iri_ref(iri_ref)?;
						context.insert_prefix(prefix.0.clone(), iri, meta.clone());
					}
				},
				(crate::Statement::Triples(t), meta) => {
					t.build(context, meta, triples)?;
				}
			}
		}

		Ok(())
	}
}

impl<M: Clone> crate::Triples<M> {
	fn build<V: RdfVocabulary + IriVocabularyMut + BlankIdVocabularyMut, G: Generator<V>>(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<(), BuildError<M>>
	where
		V::Iri: Clone,
		V::BlankId: Clone,
	{
		let subject = self.subject.build(context, triples)?;

		for (po_list, _) in self.predicate_objects_list.0.iter() {
			po_list.build(context, meta, &subject, triples)?;
		}

		Ok(())
	}
}

impl<M: Clone> crate::PredicateObjects<M> {
	fn build<V: RdfVocabulary + IriVocabularyMut + BlankIdVocabularyMut, G: Generator<V>>(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		subject: &(Subject<V::Iri, V::BlankId>, M),
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<(), BuildError<M>>
	where
		V::Iri: Clone,
		V::BlankId: Clone,
	{
		let predicate = self.verb.build(context, triples)?;

		for o in &self.objects.0 .0 {
			let object = o.build(context, triples)?;
			triples.push((
				Triple(subject.clone(), predicate.clone(), object),
				meta.clone(),
			))
		}

		Ok(())
	}
}

trait BuildFragment<M, V: RdfVocabulary + BlankIdVocabulary, G> {
	type Target;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>>;
}

impl<T: BuildMetaFragment<M, V, G>, M: Clone, V: RdfVocabulary + BlankIdVocabulary, G>
	BuildFragment<M, V, G> for (T, M)
{
	type Target = (T::Target, M);

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		Ok((self.0.build(context, &self.1, triples)?, self.1.clone()))
	}
}

trait BuildMetaFragment<M, V: RdfVocabulary, G> {
	type Target;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>>;
}

impl<M: Clone, V: RdfVocabulary + IriVocabularyMut + BlankIdVocabulary, G>
	BuildMetaFragment<M, V, G> for crate::Iri<M>
{
	type Target = V::Iri;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		_triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		match self {
			Self::IriRef(iri_ref) => context.resolve_iri_ref((iri_ref.as_iri_ref(), meta)),
			Self::PrefixedName(prefix, suffix) => {
				context.resolve_compact_iri((&prefix.0, &prefix.1), (&suffix.0, &suffix.1), meta)
			}
		}
	}
}

impl<M: Clone, V: RdfVocabulary, G: Generator<V>> BuildMetaFragment<M, V, G> for crate::BlankNode<M>
where
	V: IriVocabularyMut + BlankIdVocabularyMut,
	V::Iri: Clone,
	V::BlankId: Clone,
{
	type Target = Subject<V::Iri, V::BlankId>;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		match self {
			Self::Label(b) => Ok(Subject::Blank(context.vocabulary.insert_blank_id(b))),
			Self::Anonymous(b_property_list) => {
				let b = (context.generator.next(context.vocabulary), meta.clone());

				for (predicate_objects, meta) in b_property_list.0.iter() {
					predicate_objects.build(context, meta, &b, triples)?;
				}

				Ok(b.0)
			}
		}
	}
}

impl<M: Clone, V: RdfVocabulary, G: Generator<V>> BuildMetaFragment<M, V, G> for crate::Subject<M>
where
	V: IriVocabularyMut + BlankIdVocabularyMut,
	V::Iri: Clone,
	V::BlankId: Clone,
{
	type Target = Subject<V::Iri, V::BlankId>;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		match self {
			Self::Iri(iri) => Ok(Subject::Iri(iri.build(context, meta, triples)?)),
			Self::BlankNode(b) => Ok(b.build(context, meta, triples)?),
			Self::Collection(collection) => collection.build(context, meta, triples),
		}
	}
}
impl<M: Clone, V: RdfVocabulary, G: Generator<V>> BuildMetaFragment<M, V, G>
	for crate::Collection<M>
where
	V: IriVocabularyMut + BlankIdVocabularyMut,
	V::Iri: Clone,
	V::BlankId: Clone,
{
	type Target = Subject<V::Iri, V::BlankId>;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		let mut head = Subject::Iri(context.vocabulary.insert(RDF_NIL));

		for o in self.0.iter().rev() {
			let item = o.build(context, triples)?;
			let node = context.generator.next(context.vocabulary);

			triples.push((
				Triple(
					(node.clone(), item.1.clone()),
					(context.vocabulary.insert(RDF_TYPE), item.1.clone()),
					(
						Term::Id(Id::Iri(context.vocabulary.insert(RDF_LIST))),
						item.1.clone(),
					),
				),
				meta.clone(),
			));

			triples.push((
				Triple(
					(node.clone(), item.1.clone()),
					(context.vocabulary.insert(RDF_REST), item.1.clone()),
					(head.into_term(), item.1.clone()),
				),
				meta.clone(),
			));

			triples.push((
				Triple(
					(node.clone(), item.1.clone()),
					(context.vocabulary.insert(RDF_FIRST), item.1.clone()),
					item,
				),
				meta.clone(),
			));

			head = node;
		}

		Ok(head)
	}
}

impl<M: Clone, V: RdfVocabulary + IriVocabularyMut, G> BuildMetaFragment<M, V, G>
	for crate::Verb<M>
{
	type Target = V::Iri;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		match self {
			Self::A => Ok(context.vocabulary.insert(RDF_TYPE)),
			Self::Predicate(i) => i.build(context, meta, triples),
		}
	}
}

impl<M: Clone, V: RdfVocabulary, G: Generator<V>> BuildMetaFragment<M, V, G> for crate::Object<M>
where
	V: IriVocabularyMut + BlankIdVocabularyMut,
	V::Iri: Clone,
	V::BlankId: Clone,
{
	type Target = Object<Id<V::Iri, V::BlankId>, crate::RdfLiteral<M, V::Iri>>;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		match self {
			Self::Iri(iri) => Ok(Object::Id(Id::Iri(iri.build(context, meta, triples)?))),
			Self::BlankNode(b) => Ok(b.build(context, meta, triples)?.into_term()),
			Self::Collection(collection) => {
				Ok(collection.build(context, meta, triples)?.into_term())
			}
			Self::Literal(literal) => Ok(Object::Literal(literal.build(context, meta, triples)?)),
		}
	}
}

impl<M: Clone, V: RdfVocabulary, G> BuildMetaFragment<M, V, G> for crate::Literal<M>
where
	V: IriVocabularyMut + BlankIdVocabulary,
{
	type Target = crate::RdfLiteral<M, V::Iri>;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		match self {
			Self::Boolean(b) => b.build(context, meta, triples),
			Self::Numeric(n) => n.build(context, meta, triples),
			Self::Rdf(literal) => literal.build(context, meta, triples),
		}
	}
}

impl<M: Clone, V: RdfVocabulary, G> BuildMetaFragment<M, V, G> for bool
where
	V: IriVocabularyMut + BlankIdVocabulary,
{
	type Target = crate::RdfLiteral<M, V::Iri>;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		_triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		let s = if *self { "true" } else { "false" };

		Ok(crate::RdfLiteral {
			value: (s.to_owned(), meta.clone()),
			type_: (
				LiteralType::Any(context.vocabulary.insert(XSD_BOOLEAN)),
				meta.clone(),
			),
		})
	}
}

impl<M: Clone, V: RdfVocabulary, G> BuildMetaFragment<M, V, G> for crate::NumericLiteral
where
	V: IriVocabularyMut + BlankIdVocabulary,
{
	type Target = crate::RdfLiteral<M, V::Iri>;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		meta: &M,
		_triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		let (s, ty) = match self {
			Self::Integer(i) => (i.as_str(), XSD_INTEGER),
			Self::Decimal(d) => (d.as_str(), XSD_DECIMAL),
			Self::Double(d) => (d.as_str(), XSD_DOUBLE),
		};

		Ok(crate::RdfLiteral::new(
			(s.to_owned(), meta.clone()),
			(
				LiteralType::Any(context.vocabulary.insert(ty)),
				meta.clone(),
			),
		))
	}
}

impl<M: Clone, V: RdfVocabulary, G> BuildMetaFragment<M, V, G> for crate::RdfLiteral<M>
where
	V: IriVocabularyMut + BlankIdVocabulary,
{
	type Target = crate::RdfLiteral<M, V::Iri>;

	fn build(
		&self,
		context: &mut Context<M, V, G>,
		_meta: &M,
		triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		let type_ = match &self.type_ {
			(LiteralType::Any(t), meta) => (
				LiteralType::Any(t.build(context, meta, triples)?),
				meta.clone(),
			),
			(LiteralType::LangString(tag), meta) => (
				LiteralType::LangString(tag.build(context, meta, triples)?),
				meta.clone(),
			),
		};

		Ok(crate::RdfLiteral::new(self.value.clone(), type_))
	}
}

impl<M: Clone, V: RdfVocabulary, G> BuildMetaFragment<M, V, G> for LangTagBuf {
	type Target = LangTagBuf;

	fn build(
		&self,
		_context: &mut Context<M, V, G>,
		_meta: &M,
		_triples: &mut Vec<MetaTriple<M, V>>,
	) -> Result<Self::Target, BuildError<M>> {
		// NOTE Clone needed?
		Ok(self.clone())
	}
}
