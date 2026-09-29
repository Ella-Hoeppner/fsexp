mod ast;
pub mod document;
pub mod examples;
pub mod formatting;
mod parse;
mod parser;
pub mod str_tagged;
pub mod syntax;
pub use ast::Ast;
pub use ast::InvalidTreePath;
pub use ast::SyntaxTree;
pub use document::DocumentSyntaxTree;
pub use parse::ParseError;
pub use parser::Parser;
pub use syntax::Context;
pub use syntax::ContextId;
pub use syntax::Encloser;
pub use syntax::EncloserOrOperator;
pub use syntax::Operator;
pub mod diff;

pub fn standard_whitespace_chars() -> Vec<String> {
  vec![
    " ".to_string(),
    "\n".to_string(),
    "\t".to_string(),
    "\r".to_string(),
  ]
}

#[cfg(test)]
mod core_tests {
  use crate::{
    Ast, DocumentSyntaxTree, EncloserOrOperator, ParseError, Parser,
    ast::Sexp,
    diff::{AstDiff, AstSource},
    document::{
      Document, DocumentPosition, InvalidDocumentCharPos, InvalidDocumentIndex,
    },
    examples::{
      psuedo_clj::{CljEncloser, CljOperator, CljSyntax},
      sexp::SexpSyntax,
    },
    formatting::{AlignedToSecondFormatter, Formatter, SingleLineFormatter},
    parse::ParseErrorKind,
    standard_whitespace_chars,
    str_tagged::{
      StringTaggedEncloser, StringTaggedOperator, StringTaggedSyntax,
    },
    syntax::{ContainsEncloserOrOperator, Syntax},
  };

  fn leaf(s: String) -> Sexp {
    Sexp::leaf(s)
  }

  fn inner(subexpressions: Vec<Sexp>) -> Sexp {
    Sexp::inner(subexpressions)
  }

  fn escaped_ast_graph<'g>() -> StringTaggedSyntax<'g> {
    StringTaggedSyntax::contextless_from_descriptions(
      standard_whitespace_chars(),
      Some("\\".to_string()),
      vec![("", "(", ")")],
      vec![],
    )
  }

  fn plus_ast_graph<'g>() -> StringTaggedSyntax<'g> {
    StringTaggedSyntax::contextless_from_descriptions(
      standard_whitespace_chars(),
      Some("\\".to_string()),
      vec![("", "(", ")")],
      vec![("PLUS", "+", 1, 1)],
    )
  }

  fn question_mark_ast_graph<'g>() -> StringTaggedSyntax<'g> {
    StringTaggedSyntax::contextless_from_descriptions(
      standard_whitespace_chars(),
      Some("\\".to_string()),
      vec![("", "(", ")")],
      vec![("QMARK", "?", 1, 0)],
    )
  }

  fn pipe_ast_graph<'g>() -> StringTaggedSyntax<'g> {
    StringTaggedSyntax::contextless_from_descriptions(
      standard_whitespace_chars(),
      None,
      vec![("", "(", ")"), ("PIPE", "|", "|")],
      vec![],
    )
  }

  fn quote_ast_graph<'g>() -> StringTaggedSyntax<'g> {
    StringTaggedSyntax::contextless_from_descriptions(
      standard_whitespace_chars(),
      None,
      vec![("", "(", ")")],
      vec![("QUOTE", "'", 0, 1)],
    )
  }

  fn string_ast_graph<'g>() -> StringTaggedSyntax<'g> {
    StringTaggedSyntax::from_descriptions(
      "root",
      vec![
        (
          "root",
          false,
          vec!["", "STRING"],
          None,
          standard_whitespace_chars(),
        ),
        ("string", false, vec![], Some('\\'.to_string()), vec![]),
      ],
      vec![("", "(", ")", None), ("STRING", "\"", "\"", Some("string"))],
      vec![],
    )
  }

  fn multi_bracket_graph<'g>() -> StringTaggedSyntax<'g> {
    StringTaggedSyntax::contextless_from_descriptions(
      standard_whitespace_chars(),
      None,
      vec![
        ("", "(", ")"),
        ("SQUARE", "[", "]"),
        ("CURLY", "{", "}"),
        ("HASH_CURLY", "#{", "}"),
      ],
      vec![],
    )
  }

  #[test]
  fn ast_terminal() {
    assert_eq!(
      Parser::new(SexpSyntax, "hello!").read_next_as_sexp(),
      (Some(leaf("hello!".to_string())), vec![])
    );
  }

  #[test]
  fn ast_whitespaced_list() {
    assert_eq!(
      Parser::new(SexpSyntax, "( + 1 2 )").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("+".to_string()),
          leaf("1".to_string()),
          leaf("2".to_string())
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn ast_list() {
    assert_eq!(
      Parser::new(SexpSyntax, "(1)").read_next_as_sexp(),
      (Some(inner(vec![leaf("1".to_string())])), vec![])
    );
  }

  #[test]
  fn ast_terminal_non_whitespaced_into_opener() {
    assert_eq!(
      Parser::new(SexpSyntax, "(hello?())").read_next_as_sexp(),
      (
        Some(inner(vec![leaf("hello?".to_string()), inner(vec![])])),
        vec![]
      )
    );
  }

  #[test]
  fn ast_nested_list() {
    assert_eq!(
      Parser::new(SexpSyntax, "(+ 1 (* 2 3))").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("+".to_string()),
          leaf("1".to_string()),
          inner(vec![
            leaf("*".to_string()),
            leaf("2".to_string()),
            leaf("3".to_string())
          ]),
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn unclosed_list_causes_error() {
    assert_eq!(
      Parser::new(SexpSyntax, "(+ 1 2").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("+".to_string()),
          leaf("1".to_string()),
          leaf("2".to_string())
        ])),
        vec![ParseError {
          kind: ParseErrorKind::EndOfTextWithOpenEncloser("(".to_string()),
          pos: 0..1
        }]
      )
    );
  }

  #[test]
  fn square_bracket() {
    assert_eq!(
      Parser::new(multi_bracket_graph(), "[1 2]").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("SQUARE".to_string()),
          leaf("1".to_string()),
          leaf("2".to_string())
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn nested_brackets() {
    assert_eq!(
      Parser::new(multi_bracket_graph(), "([{#{hello!}}])").read_next_as_sexp(),
      (
        Some(inner(vec![inner(vec![
          leaf("SQUARE".to_string()),
          inner(vec![
            leaf("CURLY".to_string()),
            inner(vec![
              leaf("HASH_CURLY".to_string()),
              leaf("hello!".to_string())
            ]),
          ]),
        ])])),
        vec![]
      )
    );
  }

  #[test]
  fn nested_brackets_extra_hash() {
    assert_eq!(
      Parser::new(multi_bracket_graph(), "([{####{hello!}}])")
        .read_next_as_sexp(),
      (
        Some(inner(vec![inner(vec![
          leaf("SQUARE".to_string()),
          inner(vec![
            leaf("CURLY".to_string()),
            leaf("###".to_string()),
            inner(vec![
              leaf("HASH_CURLY".to_string()),
              leaf("hello!".to_string())
            ]),
          ]),
        ])])),
        vec![]
      )
    );
  }

  #[test]
  fn mismatched_brackets_cause_error() {
    assert_eq!(
      Parser::new(multi_bracket_graph(), "([)]").read_next_as_sexp(),
      (
        Some(inner(vec![inner(vec![leaf("SQUARE".to_string()),]),])),
        vec![
          ParseError {
            kind: ParseErrorKind::UnexpectedCloser(")".to_string()),
            pos: 2..3
          },
          ParseError {
            kind: ParseErrorKind::UnexpectedCloser("]".to_string()),
            pos: 3..4
          }
        ]
      )
    );
  }

  #[test]
  fn stray_closer_at_top_level_is_an_error() {
    assert_eq!(
      Parser::new(multi_bracket_graph(), "a) (b)]").read_all_as_sexps(),
      (
        vec![leaf("a".to_string()), inner(vec![leaf("b".to_string())]),],
        vec![
          ParseError {
            kind: ParseErrorKind::UnexpectedCloser(")".to_string()),
            pos: 1..2
          },
          ParseError {
            kind: ParseErrorKind::UnexpectedCloser("]".to_string()),
            pos: 6..7
          }
        ]
      )
    );
  }

  #[test]
  fn prefix_op() {
    assert_eq!(
      Parser::new(quote_ast_graph(), "'hello!").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("QUOTE".to_string()),
          leaf("hello!".to_string())
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn suffix_op() {
    assert_eq!(
      Parser::new(question_mark_ast_graph(), "hello?").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("QMARK".to_string()),
          leaf("hello".to_string())
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn prefix_op_in_list() {
    assert_eq!(
      Parser::new(quote_ast_graph(), "('hello! goodbye!)").read_next_as_sexp(),
      (
        Some(inner(vec![
          inner(vec![leaf("QUOTE".to_string()), leaf("hello!".to_string())]),
          leaf("goodbye!".to_string())
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn top_level_infix_op() {
    assert_eq!(
      Parser::new(plus_ast_graph(), "1+2").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("PLUS".to_string()),
          leaf("1".to_string()),
          leaf("2".to_string())
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn solo_infix_op_in_list() {
    assert_eq!(
      Parser::new(plus_ast_graph(), "(1+2)").read_next_as_sexp(),
      (
        Some(inner(vec![inner(vec![
          leaf("PLUS".to_string()),
          leaf("1".to_string()),
          leaf("2".to_string())
        ])])),
        vec![]
      )
    );
  }

  #[test]
  fn nested_infix_op_in_list() {
    assert_eq!(
      Parser::new(plus_ast_graph(), "(1+2+3)").read_next_as_sexp(),
      (
        Some(inner(vec![inner(vec![
          leaf("PLUS".to_string()),
          inner(vec![
            leaf("PLUS".to_string()),
            leaf("1".to_string()),
            leaf("2".to_string())
          ]),
          leaf("3".to_string())
        ])])),
        vec![]
      )
    );
  }

  #[test]
  fn terminals_after_infix_op_in_list() {
    assert_eq!(
      Parser::new(plus_ast_graph(), "(1+2 3)").read_next_as_sexp(),
      (
        Some(inner(vec![
          inner(vec![
            leaf("PLUS".to_string()),
            leaf("1".to_string()),
            leaf("2".to_string())
          ]),
          leaf("3".to_string())
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn op_missing_left_arg_causes_error() {
    assert_eq!(
      Parser::new(plus_ast_graph(), "(+2)").read_next_as_sexp(),
      (
        Some(inner(vec![inner(vec![
          leaf("PLUS".to_string()),
          leaf("2".to_string())
        ])])),
        vec![ParseError {
          kind: ParseErrorKind::OperatorMissingLeftArgument("+".to_string()),
          pos: 1..2
        }]
      )
    );
  }

  #[test]
  fn unfinished_infix_op_causes_error() {
    assert_eq!(
      Parser::new(plus_ast_graph(), "(1+)").read_next_as_sexp(),
      (
        Some(inner(vec![inner(vec![
          leaf("PLUS".to_string()),
          leaf("1".to_string())
        ])])),
        vec![ParseError {
          kind: ParseErrorKind::OperatorMissingRightArgument("+".to_string()),
          pos: 2..3
        }]
      )
    );
  }

  #[test]
  fn unfinished_top_level_infix_op_causes_error() {
    assert_eq!(
      Parser::new(plus_ast_graph(), "1+").read_next_as_sexp(),
      (
        Some(inner(vec![leaf("PLUS".to_string()), leaf("1".to_string())])),
        vec![ParseError {
          kind: ParseErrorKind::OperatorMissingRightArgument("+".to_string()),
          pos: 1..2
        }]
      )
    );
  }

  #[test]
  fn operator_counts_skip_rightward_comments() {
    assert_eq!(
      Parser::new(
        StringTaggedSyntax::from_descriptions(
          "root",
          vec![
            (
              "root",
              false,
              vec!["", "PLUS", "COMMENT"],
              None,
              standard_whitespace_chars(),
            ),
            (
              "comment",
              true,
              vec!["", "PLUS", "COMMENT"],
              None,
              standard_whitespace_chars(),
            )
          ],
          vec![("", "(", ")", None),],
          vec![
            ("PLUS", "+", 1, 1, None),
            ("COMMENT", "#_", 0, 1, Some("comment")),
          ]
        ),
        "1+#_2 3"
      )
      .read_all_as_sexps(),
      (
        vec![inner(vec![
          leaf("PLUS".to_string()),
          leaf("1".to_string()),
          inner(vec![leaf("COMMENT".to_string()), leaf("2".to_string())]),
          leaf("3".to_string())
        ])],
        vec![]
      )
    );
  }

  #[test]
  fn top_level_operator_counts_skip_leftward_comments() {
    assert_eq!(
      Parser::new(
        StringTaggedSyntax::from_descriptions(
          "root",
          vec![
            (
              "root",
              false,
              vec!["", "PLUS", "COMMENT"],
              None,
              standard_whitespace_chars(),
            ),
            (
              "comment",
              true,
              vec!["", "PLUS", "COMMENT"],
              None,
              standard_whitespace_chars(),
            )
          ],
          vec![("", "(", ")", None),],
          vec![
            ("PLUS", "+", 1, 1, None),
            ("COMMENT", "#_", 0, 1, Some("comment")),
          ]
        ),
        "1 #_2+3"
      )
      .read_all_as_sexps(),
      (
        vec![inner(vec![
          leaf("PLUS".to_string()),
          leaf("1".to_string()),
          inner(vec![leaf("COMMENT".to_string()), leaf("2".to_string())]),
          leaf("3".to_string())
        ])],
        vec![]
      )
    );
  }

  #[test]
  fn operator_counts_skip_leftward_comments() {
    assert_eq!(
      Parser::new(
        StringTaggedSyntax::from_descriptions(
          "root",
          vec![
            (
              "root",
              false,
              vec!["", "PLUS", "COMMENT"],
              None,
              standard_whitespace_chars(),
            ),
            (
              "comment",
              true,
              vec!["", "PLUS", "COMMENT"],
              None,
              standard_whitespace_chars(),
            )
          ],
          vec![("", "(", ")", None),],
          vec![
            ("PLUS", "+", 1, 1, None),
            ("COMMENT", "#_", 0, 1, Some("comment")),
          ]
        ),
        "(1 #_2+3)"
      )
      .read_all_as_sexps(),
      (
        vec![inner(vec![inner(vec![
          leaf("PLUS".to_string()),
          leaf("1".to_string()),
          inner(vec![leaf("COMMENT".to_string()), leaf("2".to_string())]),
          leaf("3".to_string())
        ])])],
        vec![]
      )
    );
  }

  #[test]
  fn contextful_brackets() {
    assert_eq!(
      Parser::new(
        StringTaggedSyntax::from_descriptions(
          "root",
          vec![
            (
              "root",
              false,
              vec!["", "SQUARE"],
              None,
              standard_whitespace_chars(),
            ),
            (
              "include_angle",
              false,
              vec!["", "SQUARE", "ANGLE"],
              None,
              standard_whitespace_chars(),
            )
          ],
          vec![
            ("", "(", ")", None),
            ("SQUARE", "[", "]", Some("include_angle")),
            ("ANGLE", "<", ">", Some("include_angle"))
          ],
          vec![]
        ),
        "(> < [<>])"
      )
      .read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf(">".to_string()),
          leaf("<".to_string()),
          inner(vec![
            leaf("SQUARE".to_string()),
            inner(vec![leaf("ANGLE".to_string())])
          ]),
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn contextful_operator() {
    assert_eq!(
      Parser::new(
        StringTaggedSyntax::from_descriptions(
          "root",
          vec![
            (
              "root",
              false,
              vec!["", "COLON"],
              None,
              standard_whitespace_chars(),
            ),
            (
              "include_angle",
              false,
              vec!["", "ANGLE", "COLON"],
              None,
              standard_whitespace_chars(),
            )
          ],
          vec![
            ("", "(", ")", None),
            ("ANGLE", "<", ">", Some("include_angle"))
          ],
          vec![("COLON", ":", 1, 1, Some("include_angle"))],
        ),
        "((> 1 0) : <Bool>)"
      )
      .read_next_as_sexp(),
      (
        Some(inner(vec![inner(vec![
          leaf("COLON".to_string()),
          inner(vec![
            leaf(">".to_string()),
            leaf("1".to_string()),
            leaf("0".to_string())
          ]),
          inner(vec![leaf("ANGLE".to_string()), leaf("Bool".to_string())])
        ])])),
        vec![]
      )
    );
  }

  #[test]
  fn symmetric_encloser() {
    assert_eq!(
      Parser::new(pipe_ast_graph(), "|+ 1 2|").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("PIPE".to_string()),
          leaf("+".to_string()),
          leaf("1".to_string()),
          leaf("2".to_string())
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn escaped_closer() {
    assert_eq!(
      Parser::new(escaped_ast_graph(), "(\\))").read_next_as_sexp(),
      (Some(inner(vec![leaf("\\)".to_string())])), vec![])
    );
  }

  #[test]
  fn escaped_opener() {
    assert_eq!(
      Parser::new(escaped_ast_graph(), "(\\()").read_next_as_sexp(),
      (Some(inner(vec![leaf("\\(".to_string())])), vec![])
    );
  }

  #[test]
  fn escaped_operator() {
    assert_eq!(
      Parser::new(plus_ast_graph(), "(\\+)").read_next_as_sexp(),
      (Some(inner(vec![leaf("\\+".to_string())])), vec![])
    );
  }

  #[test]
  fn symmetric_enclosers_in_list() {
    assert_eq!(
      Parser::new(pipe_ast_graph(), "(|+ 1 2| |a|)").read_next_as_sexp(),
      (
        Some(inner(vec![
          inner(vec![
            leaf("PIPE".to_string()),
            leaf("+".to_string()),
            leaf("1".to_string()),
            leaf("2".to_string())
          ]),
          inner(vec![leaf("PIPE".to_string()), leaf("a".to_string())])
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn nested_symmetric_enclosers() {
    assert_eq!(
      Parser::new(pipe_ast_graph(), "|(|a|)|").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("PIPE".to_string()),
          inner(vec![inner(vec![
            leaf("PIPE".to_string()),
            leaf("a".to_string())
          ])])
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn read_two_asts() {
    let mut parser = Parser::new(SexpSyntax, "(+ 1 2) (* 3 4)");
    assert_eq!(
      parser.read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("+".to_string()),
          leaf("1".to_string()),
          leaf("2".to_string())
        ])),
        vec![]
      )
    );
    assert_eq!(
      parser.read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("*".to_string()),
          leaf("3".to_string()),
          leaf("4".to_string())
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn read_all_single_ast() {
    assert_eq!(
      Parser::new(SexpSyntax, "(+ 1 2)").read_all_as_sexps(),
      (
        vec![inner(vec![
          leaf("+".to_string()),
          leaf("1".to_string()),
          leaf("2".to_string())
        ])],
        vec![]
      )
    );
  }

  #[test]
  fn read_all_double_ast() {
    assert_eq!(
      Parser::new(SexpSyntax, "(+ 1 2) (* 3 4)").read_all_as_sexps(),
      (
        vec![
          inner(vec![
            leaf("+".to_string()),
            leaf("1".to_string()),
            leaf("2".to_string())
          ]),
          inner(vec![
            leaf("*".to_string()),
            leaf("3".to_string()),
            leaf("4".to_string())
          ])
        ],
        vec![]
      )
    );
  }

  #[test]
  fn read_all_double_ast_err() {
    assert_eq!(
      Parser::new(SexpSyntax, "(+ 1 2) (* 3 4").read_all_as_sexps(),
      (
        vec![
          inner(vec![
            leaf("+".to_string()),
            leaf("1".to_string()),
            leaf("2".to_string())
          ]),
          inner(vec![
            leaf("*".to_string()),
            leaf("3".to_string()),
            leaf("4".to_string())
          ])
        ],
        vec![ParseError {
          kind: ParseErrorKind::EndOfTextWithOpenEncloser("(".to_string()),
          pos: 8..9
        }]
      )
    );
  }

  #[test]
  fn contextful_whitespace() {
    assert_eq!(
      Parser::new(
        string_ast_graph(),
        "(before string \" inside string!!! \" after string)"
      )
      .read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("before".to_string()),
          leaf("string".to_string()),
          inner(vec![
            leaf("STRING".to_string()),
            leaf(" inside string!!! ".to_string()),
          ]),
          leaf("after".to_string()),
          leaf("string".to_string()),
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn contextful_escape() {
    assert_eq!(
      Parser::new(string_ast_graph(), "\"\\\"\"").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("STRING".to_string()),
          leaf("\\\"".to_string()),
        ])),
        vec![]
      )
    );
  }

  #[test]
  fn solo_ast_char_indeces() {
    assert_eq!(
      Parser::new(SexpSyntax, "(+ 1 2)")
        .read_next()
        .0
        .unwrap()
        .calculate_paths(vec![0]),
      DocumentSyntaxTree::Inner(
        (
          DocumentPosition::new(0..7, vec![0]),
          EncloserOrOperator::Encloser(())
        ),
        vec![
          DocumentSyntaxTree::Leaf(
            DocumentPosition::new(1..2, vec![0, 0]),
            "+".to_string()
          ),
          DocumentSyntaxTree::Leaf(
            DocumentPosition::new(3..4, vec![0, 1]),
            "1".to_string()
          ),
          DocumentSyntaxTree::Leaf(
            DocumentPosition::new(5..6, vec![0, 2]),
            "2".to_string()
          )
        ]
      )
    )
  }

  #[test]
  fn nested_ast_char_indeces() {
    assert_eq!(
      Parser::new(SexpSyntax, "(* (+ 1 2) 3)")
        .read_next()
        .0
        .unwrap()
        .calculate_paths(vec![0]),
      DocumentSyntaxTree::Inner(
        (
          DocumentPosition::new(0..13, vec![0]),
          EncloserOrOperator::Encloser(())
        ),
        vec![
          DocumentSyntaxTree::Leaf(
            DocumentPosition::new(1..2, vec![0, 0]),
            "*".to_string()
          ),
          DocumentSyntaxTree::Inner(
            (
              DocumentPosition::new(3..10, vec![0, 1]),
              EncloserOrOperator::Encloser(())
            ),
            vec![
              DocumentSyntaxTree::Leaf(
                DocumentPosition::new(4..5, vec![0, 1, 0]),
                "+".to_string()
              ),
              DocumentSyntaxTree::Leaf(
                DocumentPosition::new(6..7, vec![0, 1, 1]),
                "1".to_string()
              ),
              DocumentSyntaxTree::Leaf(
                DocumentPosition::new(8..9, vec![0, 1, 2]),
                "2".to_string()
              )
            ]
          ),
          DocumentSyntaxTree::Leaf(
            DocumentPosition::new(11..12, vec![0, 2]),
            "3".to_string()
          ),
        ]
      )
    )
  }

  #[test]
  fn multi_bracket_ast_char_indeces() {
    assert_eq!(
      Parser::new(multi_bracket_graph(), "(union #{1 20} #{})")
        .read_next()
        .0
        .unwrap()
        .calculate_paths(vec![0]),
      DocumentSyntaxTree::Inner(
        (
          DocumentPosition::new(0..19, vec![0]),
          EncloserOrOperator::Encloser(StringTaggedEncloser::new("", "(", ")"))
        ),
        vec![
          DocumentSyntaxTree::Leaf(
            DocumentPosition::new(1..6, vec![0, 0]),
            "union".to_string()
          ),
          DocumentSyntaxTree::Inner(
            (
              DocumentPosition::new(7..14, vec![0, 1]),
              EncloserOrOperator::Encloser(StringTaggedEncloser::new(
                "HASH_CURLY",
                "#{",
                "}"
              ))
            ),
            vec![
              DocumentSyntaxTree::Leaf(
                DocumentPosition::new(9..10, vec![0, 1, 0]),
                "1".to_string()
              ),
              DocumentSyntaxTree::Leaf(
                DocumentPosition::new(11..13, vec![0, 1, 1]),
                "20".to_string()
              )
            ]
          ),
          DocumentSyntaxTree::Inner(
            (
              DocumentPosition::new(15..18, vec![0, 2]),
              EncloserOrOperator::Encloser(StringTaggedEncloser::new(
                "HASH_CURLY",
                "#{",
                "}"
              ))
            ),
            vec![]
          ),
        ]
      )
    )
  }

  #[test]
  fn infix_char_indeces() {
    assert_eq!(
      Parser::new(plus_ast_graph(), "(1+2)")
        .read_next()
        .0
        .unwrap()
        .calculate_paths(vec![0]),
      DocumentSyntaxTree::Inner(
        (
          DocumentPosition::new(0..5, vec![0]),
          EncloserOrOperator::Encloser(StringTaggedEncloser::new("", "(", ")"))
        ),
        vec![DocumentSyntaxTree::Inner(
          (
            DocumentPosition::new(1..4, vec![0, 0]),
            EncloserOrOperator::Operator(StringTaggedOperator::new(
              "PLUS", "+", 1, 1
            ))
          ),
          vec![
            DocumentSyntaxTree::Leaf(
              DocumentPosition::new(1..2, vec![0, 0, 0]),
              "1".to_string()
            ),
            DocumentSyntaxTree::Leaf(
              DocumentPosition::new(3..4, vec![0, 0, 1]),
              "2".to_string()
            )
          ]
        )]
      )
    );
  }

  #[test]
  fn ast_document_subtree() {
    let doc = Document::from_text_with_syntax(SexpSyntax, "(* (+ 1 2) 3)");
    assert_eq!(
      doc.get_subtree(&[0]).unwrap().clone(),
      Parser::new(SexpSyntax, "(* (+ 1 2) 3)")
        .read_next()
        .0
        .unwrap()
        .calculate_paths(vec![0])
    );
    assert_eq!(
      Sexp::from(doc.get_subtree(&[0, 0]).unwrap().clone()),
      Parser::new(SexpSyntax, "*").read_next_as_sexp().0.unwrap()
    );
    assert_eq!(
      Sexp::from(doc.get_subtree(&[0, 1]).unwrap().clone()),
      Parser::new(SexpSyntax, "(+ 1 2)")
        .read_next_as_sexp()
        .0
        .unwrap()
    );
  }

  #[test]
  fn infix_ast_document_subtree() {
    let doc = Document::from_text_with_syntax(plus_ast_graph(), "(inc 1 + 2)");
    assert_eq!(
      Sexp::from(doc.get_subtree(&[0, 0]).unwrap().clone()),
      Parser::new(plus_ast_graph(), "inc")
        .read_next_as_sexp()
        .0
        .unwrap()
    );
    assert_eq!(
      Sexp::from(doc.get_subtree(&[0, 1]).unwrap().clone()),
      Parser::new(plus_ast_graph(), "1 + 2")
        .read_next_as_sexp()
        .0
        .unwrap()
    );
    assert_eq!(
      Sexp::from(doc.get_subtree(&[0, 1, 0]).unwrap().clone()),
      Parser::new(plus_ast_graph(), "1")
        .read_next_as_sexp()
        .0
        .unwrap()
    );
    assert_eq!(
      Sexp::from(doc.get_subtree(&[0, 1, 1]).unwrap().clone()),
      Parser::new(plus_ast_graph(), "2")
        .read_next_as_sexp()
        .0
        .unwrap()
    );
  }

  #[test]
  fn multiple_infix_ast_document_subtree() {
    let doc = Document::from_text_with_syntax(plus_ast_graph(), "a b");
    assert_eq!(
      Sexp::from(doc.get_subtree(&[0]).unwrap().clone()),
      Parser::new(plus_ast_graph(), "a")
        .read_next_as_sexp()
        .0
        .unwrap()
    );
    assert_eq!(
      Sexp::from(doc.get_subtree(&[1]).unwrap().clone()),
      Parser::new(plus_ast_graph(), "b")
        .read_next_as_sexp()
        .0
        .unwrap()
    );
  }

  #[test]
  fn ast_document_enclosing_paths() {
    let doc = Document::from_text_with_syntax(SexpSyntax, "(* (+ 1 2) 3)");
    assert_eq!(doc.innermost_enclosing_path(&(0..0)), vec![0]);
    assert_eq!(doc.innermost_enclosing_path(&(1..1)), vec![0, 0]);
    assert_eq!(doc.innermost_enclosing_path(&(2..2)), vec![0, 0]);
    assert_eq!(doc.innermost_enclosing_path(&(3..3)), vec![0, 1]);
    assert_eq!(doc.innermost_enclosing_path(&(4..4)), vec![0, 1, 0]);
    assert_eq!(doc.innermost_enclosing_path(&(5..5)), vec![0, 1, 0]);
    assert_eq!(doc.innermost_enclosing_path(&(6..6)), vec![0, 1, 1]);
    assert_eq!(doc.innermost_enclosing_path(&(4..6)), vec![0, 1]);
    assert_eq!(doc.innermost_enclosing_path(&(5..6)), vec![0, 1]);
    assert_eq!(doc.innermost_enclosing_path(&(0..2)), vec![0]);
    assert_eq!(doc.innermost_enclosing_path(&(100..200)), vec![]);
  }

  #[test]
  fn ast_document_expand_selection() {
    let doc = Document::from_text_with_syntax(SexpSyntax, "(* (+ 1 2) 3) ");

    assert_eq!(doc.expand_selection(&(0..0)), Some(0..13));
    assert_eq!(doc.expand_selection(&(0..1)), Some(0..13));
    assert_eq!(doc.expand_selection(&(0..2)), Some(0..13));
    assert_eq!(doc.expand_selection(&(13..13)), Some(0..13));
    assert_eq!(doc.expand_selection(&(14..14)), None);

    assert_eq!(doc.expand_selection(&(1..1)), Some(1..2));
    assert_eq!(doc.expand_selection(&(1..2)), Some(0..13));

    assert_eq!(doc.expand_selection(&(3..3)), Some(3..10));
    assert_eq!(doc.expand_selection(&(3..4)), Some(3..10));
    assert_eq!(doc.expand_selection(&(3..5)), Some(3..10));
    assert_eq!(doc.expand_selection(&(3..10)), Some(0..13));

    assert_eq!(doc.expand_selection(&(4..4)), Some(4..5));
    assert_eq!(doc.expand_selection(&(4..5)), Some(3..10));

    assert_eq!(doc.expand_selection(&(0..13)), None);
  }

  #[test]
  fn two_ast_document_expand_selection() {
    let doc = Document::from_text_with_syntax(SexpSyntax, "(+ 1 2) (* 3 4)");

    assert_eq!(doc.expand_selection(&(0..0)), Some(0..7));
    assert_eq!(doc.expand_selection(&(7..7)), Some(0..7));

    assert_eq!(doc.expand_selection(&(8..8)), Some(8..15));
    assert_eq!(doc.expand_selection(&(15..15)), Some(8..15));

    assert_eq!(doc.expand_selection(&(0..7)), Some(0..15));
    assert_eq!(doc.expand_selection(&(0..15)), None);
  }

  #[test]
  fn two_touching_ast_document_expand_selection() {
    let doc = Document::from_text_with_syntax(SexpSyntax, "(+ 1 2)(* 3 4)");

    assert_eq!(doc.expand_selection(&(0..0)), Some(0..7));
    assert_eq!(doc.expand_selection(&(7..7)), Some(0..7));

    assert_eq!(doc.expand_selection(&(14..14)), Some(7..14));

    assert_eq!(doc.expand_selection(&(7..14)), Some(0..14));
  }

  #[test]
  fn plus_ast_document_expand_selection() {
    let doc = Document::from_text_with_syntax(plus_ast_graph(), "1 + 2");

    assert_eq!(doc.expand_selection(&(0..0)), Some(0..1));
    assert_eq!(doc.expand_selection(&(1..1)), Some(0..1));

    assert_eq!(doc.expand_selection(&(4..4)), Some(4..5));
    assert_eq!(doc.expand_selection(&(5..5)), Some(4..5));

    assert_eq!(doc.expand_selection(&(2..2)), Some(0..5));
    assert_eq!(doc.expand_selection(&(0..1)), Some(0..5));
  }

  #[test]
  fn ast_subtree_text() {
    let text = "(* (+ 1 2) 3)";
    let doc = Document::from_text_with_syntax(SexpSyntax, text);

    assert_eq!(doc.get_subtree_text(&[0], text).unwrap(), "(* (+ 1 2) 3)");
    assert_eq!(doc.get_subtree_text(&[0, 0], text).unwrap(), "*");
    assert_eq!(doc.get_subtree_text(&[0, 1], text).unwrap(), "(+ 1 2)");
    assert_eq!(doc.get_subtree_text(&[0, 1, 2], text).unwrap(), "2");
  }

  #[test]
  fn single_line_document_index_to_row_and_col() {
    let text = "(* (+ 1 2) 3)";
    let doc = Document::from_text_with_syntax(SexpSyntax, text);
    for i in 0..text.len() {
      assert_eq!(doc.index_to_row_and_col(i, text), Ok((0, i)));
    }
    assert_eq!(
      doc.index_to_row_and_col(text.len(), text),
      Ok((0, text.len()))
    );
    assert_eq!(
      doc.index_to_row_and_col(text.len() + 1, text),
      Err(InvalidDocumentIndex)
    );
  }

  #[test]
  fn multi_line_document_index_to_row_and_col() {
    let text = "(* (+ 1 2)\n   3\n   4)";
    let doc = Document::from_text_with_syntax(SexpSyntax, text);
    for i in 0..11 {
      assert_eq!(doc.index_to_row_and_col(i, text), Ok((0, i)));
    }
    for i in 11..16 {
      assert_eq!(doc.index_to_row_and_col(i, text), Ok((1, i - 11)));
    }
    for i in 16..20 {
      assert_eq!(doc.index_to_row_and_col(i, text), Ok((2, i - 16)));
    }
  }

  #[test]
  fn single_line_document_row_and_col_to_index() {
    let text = "(* (+ 1 2) 3)";
    let doc = Document::from_text_with_syntax(SexpSyntax, text);
    for i in 0..text.len() {
      assert_eq!(doc.row_and_col_to_index(0, i, text), Ok(i));
    }
  }

  #[test]
  fn multi_line_document_row_and_col_to_index() {
    let text = "(+ 1\n   2\n   3\n   4)";
    let doc = Document::from_text_with_syntax(SexpSyntax, text);
    for i in 0..4 {
      assert_eq!(doc.row_and_col_to_index(0, i, text), Ok(i));
    }
    assert_eq!(doc.row_and_col_to_index(0, 4, text), Ok(4));
    assert_eq!(
      doc.row_and_col_to_index(0, 5, text),
      Err(InvalidDocumentCharPos)
    );
    for i in 0..4 {
      assert_eq!(doc.row_and_col_to_index(1, i, text), Ok(5 + i));
    }
    assert_eq!(doc.row_and_col_to_index(1, 4, text), Ok(9));
    assert_eq!(
      doc.row_and_col_to_index(1, 5, text),
      Err(InvalidDocumentCharPos)
    );
    for i in 0..4 {
      assert_eq!(doc.row_and_col_to_index(2, i, text), Ok(10 + i));
    }
    assert_eq!(doc.row_and_col_to_index(2, 4, text), Ok(14));
    assert_eq!(
      doc.row_and_col_to_index(2, 5, text),
      Err(InvalidDocumentCharPos)
    );
    for i in 0..5 {
      assert_eq!(doc.row_and_col_to_index(3, i, text), Ok(15 + i));
    }
    assert_eq!(doc.row_and_col_to_index(3, 5, text), Ok(20));
    assert_eq!(
      doc.row_and_col_to_index(3, 6, text),
      Err(InvalidDocumentCharPos)
    );
  }

  #[test]
  fn document_row_and_col_to_index_inverts_index_to_row_and_col() {
    let text = "(* (+ 1 2)\n   3\n   4)\n";
    let doc = Document::from_text_with_syntax(SexpSyntax, text);
    for i in 0..text.len() {
      let (row, col) = doc.index_to_row_and_col(i, text).unwrap();
      assert_eq!(doc.row_and_col_to_index(row, col, text), Ok(i));
    }
  }

  #[test]
  fn strip_comments() {
    let mut doc = Document::from_text_with_syntax(
      CljSyntax,
      "#_(hello!!) (+ 1 2 #_3) ;(testing\n",
    );
    doc.strip_comments();
    assert!(doc.syntax_trees.len() == 1);
    let Ast::Inner((_, EncloserOrOperator::Encloser(CljEncloser::List)), _) =
      doc.syntax_trees[0].clone()
    else {
      assert!(false);
      panic!()
    };
    let Ast::Inner((_, _), children) = doc.syntax_trees.remove(0) else {
      unreachable!()
    };
    assert!(children.len() == 3)
  }

  #[test]
  fn document_paths() {
    let doc = Document::from_text_with_syntax(SexpSyntax, "(+ 1 2 (* 3 4)) 5");
    assert_eq!(
      doc
        .syntax_trees
        .iter()
        .map(|tree| tree.position().path.clone())
        .collect::<Vec<Vec<usize>>>(),
      vec![vec![0], vec![1]]
    );
    let first_tree_subtrees =
      if let Ast::Inner(_, children) = doc.syntax_trees[0].clone() {
        children
      } else {
        panic!();
      };
    assert_eq!(
      first_tree_subtrees
        .iter()
        .map(|tree| tree.position().path.clone())
        .collect::<Vec<Vec<usize>>>(),
      vec![vec![0, 0], vec![0, 1], vec![0, 2], vec![0, 3]]
    );
  }

  #[test]
  fn basic_format() {
    let source = "(+ 1 '(* 2 3) [4 5])";
    let mut doc = Document::from_text_with_syntax(CljSyntax, source);
    assert_eq!(
      source,
      SingleLineFormatter.format(doc.syntax_trees.remove(0))
    );
  }

  #[test]
  fn infix_operator_format() {
    let source = "(abs 1+2)";
    let mut doc = Document::from_text_with_syntax(plus_ast_graph(), source);
    assert_eq!(
      source,
      SingleLineFormatter.format(doc.syntax_trees.remove(0))
    );
  }

  #[test]
  fn multiline_format() {
    let source = "(+ 1\n   2)";
    let mut doc = Document::from_text_with_syntax(CljSyntax, source);
    assert_eq!(
      source,
      AlignedToSecondFormatter::default().format(doc.syntax_trees.remove(0))
    );
  }

  #[test]
  fn nested_multiline_format() {
    let source = "\
(+ (* 1
      2)
   (* 3
      4))";
    let mut doc = Document::from_text_with_syntax(CljSyntax, source);
    assert_eq!(
      source,
      AlignedToSecondFormatter::default().format(doc.syntax_trees.remove(0))
    );
  }

  fn stripped_syntax_trees<S: Syntax>(
    doc: Document<S>,
  ) -> Vec<Ast<(), EncloserOrOperator<S::E, S::O>>> {
    doc
      .syntax_trees
      .into_iter()
      .map(|tree| {
        tree
          .map_owned(&|_, leaf| ((), leaf), &|a| a.into_encloser_or_operator())
      })
      .collect::<Vec<_>>()
  }

  fn test_diff(
    base_source: &str,
    target_source: &str,
    diffs: Vec<
      AstDiff<
        DocumentPosition,
        (
          DocumentPosition,
          EncloserOrOperator<CljEncloser, CljOperator>,
        ),
      >,
    >,
  ) {
    let mut base_document =
      Document::from_text_with_syntax(CljSyntax, base_source);
    for diff in AstDiff::sequentialize(diffs).unwrap() {
      diff.apply(&mut base_document.syntax_trees).unwrap();
    }
    let result_document =
      Document::from_text_with_syntax(CljSyntax, target_source);
    assert_eq!(
      stripped_syntax_trees(base_document),
      stripped_syntax_trees(result_document)
    )
  }

  #[test]
  fn diff_insert() {
    test_diff(
      "",
      "5",
      vec![AstDiff::insert(
        vec![0],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "5 (+ (* 1 2) (* 3 4))",
      vec![AstDiff::insert(
        vec![0],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(5 + (* 1 2) (* 3 4))",
      vec![AstDiff::insert(
        vec![0, 0],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ (* 1 2) (* 3 4)) 5",
      vec![AstDiff::insert(
        vec![1],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ (* 1 2) (* 3 4)) (+ (* 1 2) (* 3 4))",
      vec![AstDiff::insert([1], [0])],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ (* 1 2) (* 3 4)) (* 1 2)",
      vec![AstDiff::insert([1], [0, 1])],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ (* 1 2) (* 3 4)) 5 (+ (* 1 2) (* 3 4))",
      vec![
        AstDiff::insert(
          vec![1],
          AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
        ),
        AstDiff::insert([1], [0]),
      ],
    );
  }

  #[test]
  fn diff_delete() {
    test_diff("(+ (* 1 2) (* 3 4))", "", vec![AstDiff::delete([0])]);
    test_diff("1 2 3", "2 3", vec![AstDiff::delete([0])]);
    test_diff(
      "1 2 3",
      "2",
      vec![AstDiff::delete([0]), AstDiff::delete([2])],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ (* 3 4))",
      vec![AstDiff::delete([0, 1])],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ (1 2) (* 3 4))",
      vec![AstDiff::delete([0, 1, 0])],
    );
  }

  #[test]
  fn diff_replace() {
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "5",
      vec![AstDiff::replace(
        vec![0],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ 5 (* 3 4))",
      vec![AstDiff::replace(
        vec![0, 1],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ (* 1 (+ (* 1 2) (* 3 4))) (* 3 4))",
      vec![AstDiff::replace([0, 1, 2], [0])],
    );
  }

  #[test]
  fn diff_insert_snippet() {
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ (- (* 1 2)) (* 3 4))",
      vec![AstDiff::insert_snippet(
        vec![0, 1],
        AstSource::New(Parser::new(CljSyntax, "(- _)").read_next().0.unwrap()),
        vec![1],
      )],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ (- (- (* 1 2))) (* 3 4))",
      vec![AstDiff::insert_snippet(
        vec![0, 1],
        AstSource::New(
          Parser::new(CljSyntax, "(- (- _))").read_next().0.unwrap(),
        ),
        vec![1, 1],
      )],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(- (+ (* 1 2) (* 3 4)))",
      vec![AstDiff::insert_snippet(
        vec![0],
        AstSource::New(Parser::new(CljSyntax, "(- _)").read_next().0.unwrap()),
        vec![1],
      )],
    );
  }

  #[test]
  fn diff_delete_snippet() {
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+ 1 (* 3 4))",
      vec![AstDiff::delete_snippet([0, 1], vec![1])],
    );
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "1",
      vec![AstDiff::delete_snippet([0], vec![1, 1])],
    );
  }

  #[test]
  fn diff_simultaneous_deletes() {
    test_diff(
      "(+ (* 1 2) (* 3 4))",
      "(+)",
      vec![AstDiff::delete([0, 1]), AstDiff::delete([0, 2])],
    );
  }

  fn test_diff_inverse(
    base_source: &str,
    diffs: Vec<
      AstDiff<
        DocumentPosition,
        (
          DocumentPosition,
          EncloserOrOperator<CljEncloser, CljOperator>,
        ),
      >,
    >,
  ) {
    let base_document = Document::from_text_with_syntax(CljSyntax, base_source);
    let mut modified_document = base_document.clone();
    let mut inverse_diffs = vec![];

    for diff in AstDiff::sequentialize(diffs).unwrap() {
      inverse_diffs.push(
        diff
          .inverse::<Ast<_, _>>(&modified_document.syntax_trees)
          .unwrap(),
      );
      diff.apply(&mut modified_document.syntax_trees).unwrap();
    }
    while let Some(diff) = inverse_diffs.pop() {
      diff.apply(&mut modified_document.syntax_trees).unwrap();
    }
    assert_eq!(
      stripped_syntax_trees(base_document),
      stripped_syntax_trees(modified_document)
    )
  }

  #[test]
  fn diff_insert_inverse() {
    test_diff_inverse(
      "",
      vec![AstDiff::insert(
        vec![0],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::insert(
        vec![0],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::insert(
        vec![0, 0],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::insert(
        vec![1],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff_inverse("(+ (* 1 2) (* 3 4))", vec![AstDiff::insert([1], [0])]);
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::insert([1], [0, 1])],
    );
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![
        AstDiff::insert([1], [0]),
        AstDiff::insert(
          vec![1],
          AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
        ),
      ],
    );
  }

  #[test]
  fn diff_delete_inverse() {
    test_diff_inverse("(+ (* 1 2) (* 3 4))", vec![AstDiff::delete([0])]);
    test_diff_inverse("1 2 3", vec![AstDiff::delete([0])]);
    test_diff_inverse(
      "1 2 3",
      vec![AstDiff::delete([0]), AstDiff::delete([1])],
    );
    test_diff_inverse("(+ (* 1 2) (* 3 4))", vec![AstDiff::delete([0, 1])]);
    test_diff_inverse("(+ (* 1 2) (* 3 4))", vec![AstDiff::delete([0, 1, 0])]);
  }

  #[test]
  fn diff_replace_inverse() {
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::replace(
        vec![0],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::replace(
        vec![0, 1],
        AstSource::New(Parser::new(CljSyntax, "5").read_next().0.unwrap()),
      )],
    );
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::replace([0, 1, 2], [0])],
    );
  }

  #[test]
  fn diff_insert_snippet_inverse() {
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::insert_snippet(
        [0, 1],
        AstSource::New(Parser::new(CljSyntax, "(- _)").read_next().0.unwrap()),
        vec![1],
      )],
    );
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::insert_snippet(
        [0, 1],
        AstSource::New(
          Parser::new(CljSyntax, "(- (- _))").read_next().0.unwrap(),
        ),
        vec![1, 1],
      )],
    );
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::insert_snippet(
        [0],
        AstSource::New(Parser::new(CljSyntax, "(- _)").read_next().0.unwrap()),
        vec![1],
      )],
    );
  }

  #[test]
  fn diff_delete_snippet_inverse() {
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::delete_snippet([0, 1], [1])],
    );
    test_diff_inverse(
      "(+ (* 1 2) (* 3 4))",
      vec![AstDiff::delete_snippet([0], [1, 1])],
    );
  }

  #[test]
  fn diff_move_inverse() {
    test_diff_inverse(
      "(+ a b)",
      vec![AstDiff::insert([0, 3], [0, 1]), AstDiff::delete([0, 1])],
    );
  }

  #[test]
  fn reserved_tokens_encloser() {
    let syntax = StringTaggedSyntax::contextless_from_descriptions(
      standard_whitespace_chars(),
      None,
      vec![("", "|", "|")],
      vec![],
    )
    .with_reserved_tokens(["||"]);
    assert_eq!(
      Parser::new(syntax, "|a || b|").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("a".to_string()),
          leaf("||".to_string()),
          leaf("b".to_string())
        ])),
        vec![]
      )
    )
  }

  #[test]
  fn reserved_tokens_operator() {
    let syntax = StringTaggedSyntax::contextless_from_descriptions(
      standard_whitespace_chars(),
      None,
      vec![("", "(", ")")],
      vec![("REF", "&", 0, 1)],
    )
    .with_reserved_tokens(["&&"]);
    assert_eq!(
      Parser::new(syntax.clone(), "(&& a b)").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("&&".to_string()),
          leaf("a".to_string()),
          leaf("b".to_string())
        ])),
        vec![]
      )
    );
    assert_eq!(
      Parser::new(syntax.clone(), "(&& a &b)").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("&&".to_string()),
          leaf("a".to_string()),
          inner(vec![leaf("REF".to_string()), leaf("b".to_string())])
        ])),
        vec![]
      )
    );
    assert_eq!(
      Parser::new(syntax, "(&& a & &b)").read_next_as_sexp(),
      (
        Some(inner(vec![
          leaf("&&".to_string()),
          leaf("a".to_string()),
          inner(vec![
            leaf("REF".to_string()),
            inner(vec![leaf("REF".to_string()), leaf("b".to_string())])
          ])
        ])),
        vec![]
      )
    );
  }
}
