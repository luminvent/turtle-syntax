use nquads_syntax::Parse;
use rdf_types::{LexicalTriple, RdfDisplay};
use turtle_syntax::Parse as ParseNQuads;

struct Test {
	input: &'static str,
	expected_output: &'static str,
}

impl Test {
	pub fn run(self) {
		let ast = turtle_syntax::Document::parse_str(
			&std::fs::read_to_string(self.input).unwrap(),
			|span| span,
		)
		.unwrap();
		let generator = rdf_types::generator::Blank::new();
		let mut triples: Vec<_> = ast
			.0
			.build_lexical_triples(None, generator)
			.unwrap();

		triples.sort();
		triples.dedup();

		let mut expected_triples: Vec<_> = nquads_syntax::Document::parse_str(
			&std::fs::read_to_string(self.expected_output).unwrap(),
		)
		.unwrap()
		.into_value()
		.into_iter()
		.map(|q| q.into_value().into_triple().0)
		.map(|triple| LexicalTriple::new(triple.0 .0, triple.1 .0, triple.2 .0))
		.collect();

		expected_triples.sort();

		let eq = triples == expected_triples;
		if !eq {
			for t in &triples {
				println!("{} .", t.rdf_display())
			}
		}
	}
}

macro_rules! positive_test {
	($($id:ident),*) => {
		$(
			#[test]
			fn $id () {
				Test {
					input: concat!("tests/positive/", stringify!($id) ,".ttl"),
					expected_output: concat!("tests/positive/", stringify!($id) ,".nq"),
				}.run()
			}
		)*
	};
}

positive_test! {
	p01,
	p02,
	p03,
	p04,
	p05,
	p06,
	p07,
	p08,
	p09,
	p10,
	p11,
	p12,
	p13,
	p14,
	p15,
	p16,
	p17,
	p18,
	p19,
	p20,
	p21,
	p22,
	p23,
	p24,
	p25,
	p26,
	p27,
	p28,
	p29
}
