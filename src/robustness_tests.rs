//! Parsing must never panic or hang, whatever the input, and every position
//! it reports must be a valid index into the text. These tests drive the
//! parser with deterministically generated inputs built from the tokens most
//! likely to confuse it.

use std::sync::LazyLock;

use crate::{
  Context, ContextId, Encloser, Operator, Parser,
  document::{Document, DocumentSyntaxTree},
  examples::psuedo_clj::CljSyntax,
  standard_whitespace_chars,
  syntax::Syntax,
};

/// A syntax shaped like easl's: bracket enclosers, line and block comments
/// sharing the `;` prefix, strings, an expression-comment operator, and
/// operators with left and right arguments.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Ctx {
  Default,
  StructuredComment,
  UnstructuredComment,
  String,
}

impl ContextId for Ctx {
  fn is_comment(&self) -> bool {
    matches!(self, Ctx::StructuredComment | Ctx::UnstructuredComment)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Enc {
  Parens,
  Square,
  Curly,
  LineComment,
  BlockComment,
  Quote,
}

impl Encloser for Enc {
  fn opening_encloser_str(&self) -> &str {
    match self {
      Enc::Parens => "(",
      Enc::Square => "[",
      Enc::Curly => "{",
      Enc::LineComment => ";",
      Enc::BlockComment => ";*",
      Enc::Quote => "\"",
    }
  }
  fn closing_encloser_str(&self) -> &str {
    match self {
      Enc::Parens => ")",
      Enc::Square => "]",
      Enc::Curly => "}",
      Enc::LineComment => "\n",
      Enc::BlockComment => "*;",
      Enc::Quote => "\"",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Op {
  Annotation,
  Ascription,
  ExpressionComment,
  Into,
}

impl Operator for Op {
  fn left_args(&self) -> usize {
    match self {
      Op::Ascription => 1,
      _ => 0,
    }
  }
  fn right_args(&self) -> usize {
    match self {
      Op::Annotation => 2,
      _ => 1,
    }
  }
  fn op_str(&self) -> &str {
    match self {
      Op::Annotation => "@",
      Op::Ascription => ":",
      Op::ExpressionComment => "#_",
      Op::Into => "~",
    }
  }
}

static DEFAULT_CONTEXT: LazyLock<Context<Enc, Op>> = LazyLock::new(|| {
  Context::new(
    vec![
      Enc::Parens,
      Enc::Square,
      Enc::Curly,
      Enc::BlockComment,
      Enc::LineComment,
      Enc::Quote,
    ],
    vec![
      Op::Annotation,
      Op::Ascription,
      Op::ExpressionComment,
      Op::Into,
    ],
    None,
    standard_whitespace_chars(),
  )
});

static STRING_CONTEXT: LazyLock<Context<Enc, Op>> = LazyLock::new(|| {
  Context::new(vec![], vec![], Some("\\".to_string()), vec![])
});

static TRIVIAL_CONTEXT: LazyLock<Context<Enc, Op>> =
  LazyLock::new(Context::trivial);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EaslLikeSyntax;

impl Syntax for EaslLikeSyntax {
  type C = Ctx;
  type E = Enc;
  type O = Op;
  fn root_context(&self) -> Ctx {
    Ctx::Default
  }
  fn context<'a>(&'a self, id: &Ctx) -> &'a Context<Enc, Op> {
    match id {
      Ctx::Default | Ctx::StructuredComment => &DEFAULT_CONTEXT,
      Ctx::String => &STRING_CONTEXT,
      Ctx::UnstructuredComment => &TRIVIAL_CONTEXT,
    }
  }
  fn encloser_context(&self, encloser: &Enc) -> Option<Ctx> {
    match encloser {
      Enc::LineComment | Enc::BlockComment => Some(Ctx::UnstructuredComment),
      Enc::Quote => Some(Ctx::String),
      _ => None,
    }
  }
  fn operator_context(&self, operator: &Op) -> Option<Ctx> {
    match operator {
      Op::ExpressionComment => Some(Ctx::StructuredComment),
      _ => None,
    }
  }
  fn reserved_tokens(&self) -> impl Iterator<Item = &str> {
    ["||"].into_iter()
  }
}

const TOKENS: &[&str] = &[
  "(", ")", "[", "]", "{", "}", ";", ";*", "*;", "\"", "\\", "#_", "#{", "#(",
  "/*", "*/", "@", ":", "~", "||", "|", "\n", "\r\n", " ", "\t", "a", "bc",
  "1.", "é", "🎨", "e\u{301}", "*", "#", "_",
];

/// A small deterministic generator (xorshift), so failures reproduce.
struct Rng(u64);

impl Rng {
  fn next(&mut self) -> u64 {
    self.0 ^= self.0 << 13;
    self.0 ^= self.0 >> 7;
    self.0 ^= self.0 << 17;
    self.0
  }
  fn below(&mut self, n: usize) -> usize {
    (self.next() % n as u64) as usize
  }
}

fn generate(rng: &mut Rng) -> String {
  let length = rng.below(24);
  (0..length)
    .map(|_| TOKENS[rng.below(TOKENS.len())])
    .collect()
}

fn check_tree<E: Encloser, O: Operator>(
  tree: &DocumentSyntaxTree<E, O>,
  text: &str,
) {
  let span = &tree.position().span;
  assert!(
    span.start <= span.end && span.end <= text.len(),
    "tree span {span:?} out of bounds for {text:?}"
  );
  assert!(
    text.is_char_boundary(span.start) && text.is_char_boundary(span.end),
    "tree span {span:?} splits a character in {text:?}"
  );
  if let DocumentSyntaxTree::Inner(_, children) = tree {
    for child in children {
      check_tree(child, text);
    }
  }
}

fn check_document<S: Syntax + Clone>(syntax: S, text: &str) {
  let mut document = Document::from_text_with_syntax(syntax, text);
  for tree in document.syntax_trees.iter() {
    check_tree(tree, text);
  }
  for error in document.parsing_failures.iter() {
    assert!(
      error.pos.start <= error.pos.end
        && error.pos.end <= text.len()
        && text.is_char_boundary(error.pos.start)
        && text.is_char_boundary(error.pos.end),
      "error span {:?} invalid for {text:?}",
      error.pos
    );
    error.describe(&document, text);
  }
  document.strip_comments();
  for tree in document.syntax_trees.iter() {
    check_tree(tree, text);
  }
}

fn fuzz<S: Syntax + Clone>(syntax: S, seed: u64, cases: usize) {
  let mut rng = Rng(seed);
  for _ in 0..cases {
    let text = generate(&mut rng);
    let outcome =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_document(syntax.clone(), &text)
      }));
    if outcome.is_err() {
      panic!("parsing {text:?} panicked");
    }
  }
}

#[test]
fn easl_like_syntax_never_panics() {
  fuzz(EaslLikeSyntax, 0x9e3779b97f4a7c15, 20_000);
}

#[test]
fn clj_syntax_never_panics() {
  fuzz(CljSyntax, 0x2545f4914f6cdd1d, 20_000);
}

#[test]
fn every_prefix_of_nested_text_parses() {
  let text = "(a [b {c \"d\\\" e\" ;* f\r\n g *; ; h\n}] @i j k: l #_m ~n) 🎨é";
  for (end, _) in text
    .char_indices()
    .chain(std::iter::once((text.len(), 'x')))
  {
    check_document(EaslLikeSyntax, &text[..end]);
  }
}

#[test]
fn line_comment_ends_at_crlf() {
  let (trees, errors) = Parser::new(EaslLikeSyntax, "; note\r\na").read_all();
  assert!(errors.is_empty(), "{errors:?}");
  assert_eq!(trees.len(), 2, "{trees:?}");
}

#[test]
fn crlf_separates_terminals() {
  let (trees, errors) = Parser::new(EaslLikeSyntax, "a\r\nb").read_all();
  assert!(errors.is_empty(), "{errors:?}");
  assert_eq!(trees.len(), 2, "{trees:?}");
}

#[test]
fn crlf_counts_as_a_line_break() {
  let text = "a\r\nb";
  let document = Document::from_text_with_syntax(EaslLikeSyntax, text);
  assert_eq!(document.index_to_row_and_col(3, text), Ok((1, 0)));
}

#[test]
fn subtree_text_after_multibyte_characters() {
  let text = "(é 🎨 (a b))";
  let document = Document::from_text_with_syntax(EaslLikeSyntax, text);
  assert_eq!(document.get_subtree_text(&[0, 2], text), Ok("(a b)"));
}

#[test]
fn combining_mark_after_opener_stays_in_the_text() {
  let text = "(\u{301}a)";
  let (trees, errors) = Parser::new(EaslLikeSyntax, text).read_all();
  assert!(errors.is_empty(), "{errors:?}");
  let DocumentSyntaxTree::Inner(_, children) = &trees[0] else {
    panic!("expected a list: {trees:?}")
  };
  let DocumentSyntaxTree::Leaf(_, leaf) = &children[0] else {
    panic!("expected a leaf: {children:?}")
  };
  assert_eq!(leaf, "\u{301}a");
}
