use std::{
  fmt::{Debug, Display},
  ops::Range,
};

use crate::{
  ContextId,
  document::{Document, DocumentPosition, DocumentSyntaxTree},
  syntax::{Encloser, EncloserOrOperator, Operator, Syntax},
};

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum ParseErrorKind {
  EndOfTextWithOpenEncloser(String),
  UnexpectedCloser(String),
  OperatorMissingLeftArgument(String),
  OperatorMissingRightArgument(String),
}

impl Display for ParseErrorKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    use ParseErrorKind::*;
    match self {
      EndOfTextWithOpenEncloser(opener) => {
        write!(f, "Opener \"{opener}\" was never closed")
      }
      UnexpectedCloser(closer) => {
        write!(f, "Encountered unmatched closer \"{closer}\"")
      }
      OperatorMissingLeftArgument(operator) => {
        write!(f, "Operator \"{operator}\" is missing its left argument")
      }
      OperatorMissingRightArgument(operator) => {
        write!(f, "Operator \"{operator}\" is missing its right argument")
      }
    }
  }
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct ParseError {
  pub pos: Range<usize>,
  pub kind: ParseErrorKind,
}

impl ParseError {
  pub fn describe(
    &self,
    document: &Document<impl Syntax>,
    text: &str,
  ) -> String {
    document.describe_document_position(self.pos.clone(), text)
      + "\n"
      + &format!("{}", self.kind)
  }
}

pub struct OpenAst<S: Syntax> {
  opening_index: usize,
  encloser_or_operator_index: usize,
  encloser_or_operator: EncloserOrOperator<S::E, S::O>,
  subtrees: Vec<DocumentSyntaxTree<S::E, S::O>>,
  left_args_consumed: usize,
}

pub(crate) struct Parse<'t, 's, S: Syntax> {
  text: &'t str,
  syntax: &'s S,
  inherited_top_level_asts: Vec<DocumentSyntaxTree<S::E, S::O>>,
  open_asts: Vec<OpenAst<S>>,
}

impl<'t, 's, S: Syntax> Parse<'t, 's, S> {
  pub(crate) fn new(
    inherited_top_level_asts: Vec<DocumentSyntaxTree<S::E, S::O>>,
    syntax: &'s S,
    text: &'t str,
  ) -> Self {
    Self {
      text,
      syntax,
      inherited_top_level_asts,
      open_asts: vec![],
    }
  }
  fn active_context(&self) -> S::C {
    self
      .open_asts
      .iter()
      .rev()
      .find_map(
        |OpenAst {
           encloser_or_operator,
           ..
         }| {
          self
            .syntax
            .encloser_or_operator_context(encloser_or_operator)
        },
      )
      .unwrap_or(self.syntax.root_context())
  }
  fn consume_left_asts(
    &mut self,
    operator: &S::O,
    character_index: usize,
  ) -> (Vec<DocumentSyntaxTree<S::E, S::O>>, Vec<ParseError>) {
    let mut remaining_args = operator.left_args();
    if remaining_args == 0 {
      (vec![], vec![])
    } else {
      let subtrees =
        if let Some(OpenAst { subtrees, .. }) = self.open_asts.last_mut() {
          subtrees
        } else {
          &mut self.inherited_top_level_asts
        };
      let mut consumed_args = vec![];
      let mut errors = vec![];
      while remaining_args > 0 {
        if let Some(ast) = subtrees.pop() {
          remaining_args -=
            if let DocumentSyntaxTree::Inner((_, encloser_or_operator), _) =
              &ast
              && self
                .syntax
                .encloser_or_operator_context(encloser_or_operator)
                .map(|ctx| ctx.is_comment())
                .unwrap_or(false)
            {
              0
            } else {
              1
            };
          consumed_args.push(ast);
        } else {
          errors.push(ParseError {
            kind: ParseErrorKind::OperatorMissingLeftArgument(
              operator.op_str().to_string(),
            ),
            pos: (character_index)..(character_index + operator.op_str().len()),
          });
          break;
        }
      }
      consumed_args.reverse();
      (consumed_args, errors)
    }
  }
  fn close_ast(
    &mut self,
    closing_index: usize,
  ) -> Option<DocumentSyntaxTree<S::E, S::O>> {
    let OpenAst {
      opening_index,
      encloser_or_operator,
      subtrees,
      ..
    } = self
      .open_asts
      .pop()
      .expect("called close_ast with no open partial Ast");
    self.push_closed_ast(DocumentSyntaxTree::Inner(
      (
        DocumentPosition::new(opening_index..closing_index, vec![]),
        encloser_or_operator,
      ),
      subtrees,
    ))
  }
  fn push_closed_ast(
    &mut self,
    ast: DocumentSyntaxTree<S::E, S::O>,
  ) -> Option<DocumentSyntaxTree<S::E, S::O>> {
    if let Some(OpenAst {
      encloser_or_operator,
      subtrees,
      left_args_consumed,
      ..
    }) = self.open_asts.last_mut()
    {
      let end = ast.position().end();
      subtrees.push(ast);
      if let EncloserOrOperator::Operator(operator) = encloser_or_operator {
        let noncomment_right_subtree_count: usize = subtrees
          .iter()
          .skip(*left_args_consumed)
          .map(|s| {
            if let DocumentSyntaxTree::Inner((_, encloser_or_operator), _) = s
              && let Some(ctx) = self
                .syntax
                .encloser_or_operator_context(encloser_or_operator)
              && ctx.is_comment()
            {
              0
            } else {
              1
            }
          })
          .sum();
        if noncomment_right_subtree_count == operator.right_args() {
          self.close_ast(end)
        } else {
          None
        }
      } else {
        None
      }
    } else {
      Some(ast)
    }
  }
  fn awaited_closer(&self) -> Option<&str> {
    self
      .open_asts
      .iter()
      .rev()
      .filter_map(
        |OpenAst {
           encloser_or_operator,
           ..
         }| {
          if let EncloserOrOperator::Encloser(encloser) = encloser_or_operator {
            Some(encloser.closing_encloser_str())
          } else {
            None
          }
        },
      )
      .next()
  }
  fn close_unfinished_operators(
    &mut self,
    closing_index: usize,
  ) -> Vec<ParseError> {
    let mut errors = vec![];
    while let Some(OpenAst {
      encloser_or_operator: EncloserOrOperator::Operator(operator),
      encloser_or_operator_index,
      ..
    }) = self.open_asts.last()
    {
      errors.push(ParseError {
        kind: ParseErrorKind::OperatorMissingRightArgument(
          operator.op_str().to_string(),
        ),
        pos: *encloser_or_operator_index
          ..(encloser_or_operator_index + operator.op_str().len()),
      });
      self.close_ast(closing_index);
    }
    errors
  }
  pub(crate) fn complete(
    mut self,
    already_parsed_index: usize,
  ) -> (Vec<DocumentSyntaxTree<S::E, S::O>>, bool, Vec<ParseError>) {
    let mut errors = vec![];
    let beginning_index = self
      .inherited_top_level_asts
      .last()
      .map(|syntax_tree| syntax_tree.position().end())
      .unwrap_or(already_parsed_index);
    if beginning_index >= self.text.len() {
      return (self.inherited_top_level_asts, true, vec![]);
    }

    let text = self.text;
    let mut indexed_characters = text[beginning_index..]
      .char_indices()
      .map(|(offset, c)| {
        let start = beginning_index + offset;
        (offset, &text[start..start + c.len_utf8()])
      })
      .chain(std::iter::once((text.len() - beginning_index, " ")))
      .peekable();

    let mut current_terminal_beginning: Option<usize> = None;
    let mut escaped = false;

    macro_rules! close_ast_and_maybe_return {
      ($character_index:expr, $finished:expr) => {
        if let Some(completed_ast) = self.close_ast($character_index) {
          let mut top_level_asts = self.inherited_top_level_asts;
          top_level_asts.push(completed_ast);
          return (top_level_asts, $finished, errors);
        }
      };
    }

    macro_rules! push_ast_and_maybe_return {
      ($ast:expr) => {
        if let Some(completed_ast) = self.push_closed_ast($ast) {
          let mut top_level_asts = self.inherited_top_level_asts;
          top_level_asts.push(completed_ast);
          return (top_level_asts, false, errors);
        }
      };
    }

    'outer: while let Some((character_index_offset, character)) =
      indexed_characters.next()
    {
      let character_index = beginning_index + character_index_offset;

      macro_rules! finish_terminal {
        () => {
          if let Some(terminal_beginning) = current_terminal_beginning {
            push_ast_and_maybe_return!(DocumentSyntaxTree::Leaf(
              DocumentPosition::new(
                terminal_beginning..character_index,
                vec![],
              ),
              self.text[terminal_beginning..character_index].to_string(),
            ));
            current_terminal_beginning = None;
          }
        };
      }
      // Advances past the rest of a marker `$n` bytes long that starts at
      // the current character.
      macro_rules! skip_bytes {
        ($n:expr) => {
          let marker_end = character_index + $n;
          while indexed_characters
            .peek()
            .is_some_and(|(offset, _)| beginning_index + offset < marker_end)
          {
            indexed_characters.next();
          }
        };
      }

      let active_context = self.syntax.context(&self.active_context());

      if escaped {
        escaped = false;
      } else if active_context.escape_char.as_deref() == Some(character) {
        escaped = true;
        current_terminal_beginning =
          current_terminal_beginning.or(Some(character_index));
      } else if active_context.is_whitespace(character) {
        finish_terminal!();
      } else {
        let remaining_text = &self.text[character_index..];

        let active_context_tag = self.active_context();

        // Markers match longest-first, so one that's a prefix of another
        // (`;` and `;*`) never shadows it, whatever order they're listed in.
        // Reserved tokens only split text in contexts with structure (some
        // encloser or operator), so strings and plain comments keep them.
        let structured_context = !active_context.enclosers().is_empty()
          || !active_context.operators().is_empty();
        if structured_context
          && let Some(reserved_token) = self
            .syntax
            .reserved_tokens()
            .filter(|token| remaining_text.starts_with(token))
            .max_by_key(|token| token.len())
        {
          let reserved_token = reserved_token.to_string();
          finish_terminal!();
          push_ast_and_maybe_return!(DocumentSyntaxTree::Leaf(
            DocumentPosition::new(
              character_index..character_index + reserved_token.len(),
              vec![],
            ),
            reserved_token.clone(),
          ));
          skip_bytes!(reserved_token.len());
          continue;
        }

        if let Some(awaited_closer) = self.awaited_closer()
          && remaining_text.starts_with(awaited_closer)
        {
          let closer_len = awaited_closer.len();
          finish_terminal!();
          errors.append(&mut self.close_unfinished_operators(character_index));

          if let Some(completed_ast) =
            self.close_ast(character_index + closer_len)
          {
            let mut top_level_asts = self.inherited_top_level_asts;
            top_level_asts.push(completed_ast);
            return (top_level_asts, false, errors);
          } else {
            skip_bytes!(closer_len);
            continue;
          }
        }

        let context = self.syntax.context(&active_context_tag);
        let opener = context
          .enclosers()
          .iter()
          .filter(|encloser| {
            remaining_text.starts_with(encloser.opening_encloser_str())
          })
          .max_by_key(|encloser| encloser.opening_encloser_str().len())
          .cloned();
        let operator = context
          .operators()
          .iter()
          .filter(|operator| remaining_text.starts_with(operator.op_str()))
          .max_by_key(|operator| operator.op_str().len())
          .cloned();
        let operator = operator.filter(|operator| {
          opener.as_ref().is_none_or(|opener| {
            operator.op_str().len() > opener.opening_encloser_str().len()
          })
        });

        if let Some(operator) = operator {
          let op_len = operator.op_str().len();
          finish_terminal!();
          let (leftward_args, mut consumption_errors) =
            self.consume_left_asts(&operator, character_index);
          errors.append(&mut consumption_errors);
          let right_args = operator.right_args();
          self.open_asts.push(OpenAst {
            opening_index: leftward_args
              .first()
              .map(|first_arg| first_arg.position().start())
              .unwrap_or(character_index),
            encloser_or_operator: EncloserOrOperator::Operator(operator),
            encloser_or_operator_index: character_index,
            left_args_consumed: leftward_args.len(),
            subtrees: leftward_args,
          });
          skip_bytes!(op_len);
          if right_args == 0 {
            close_ast_and_maybe_return!(character_index + op_len, false)
          }
          continue 'outer;
        }

        if let Some(encloser) = opener {
          let opener_len = encloser.opening_encloser_str().len();
          finish_terminal!();
          self.open_asts.push(OpenAst {
            opening_index: character_index,
            encloser_or_operator_index: character_index,
            encloser_or_operator: EncloserOrOperator::Encloser(encloser),
            subtrees: vec![],
            left_args_consumed: 0,
          });
          skip_bytes!(opener_len);
          continue 'outer;
        }

        if let Some(closer) = self
          .syntax
          .prefix_closer_in_context(&active_context_tag, remaining_text)
        {
          let closer_len = closer.len();
          finish_terminal!();
          errors.push(ParseError {
            kind: ParseErrorKind::UnexpectedCloser(closer.to_string()),
            pos: character_index..(character_index + closer_len),
          });
          // A mismatched closer still closes the innermost open encloser. With
          // no encloser open, it's just skipped.
          if self.awaited_closer().is_some() {
            errors
              .append(&mut self.close_unfinished_operators(character_index));
            close_ast_and_maybe_return!(character_index + closer_len, false);
          }
          skip_bytes!(closer_len);
          continue;
        }

        if current_terminal_beginning.is_none() {
          current_terminal_beginning = Some(character_index)
        }
      }
    }

    if self.open_asts.is_empty() {
      (self.inherited_top_level_asts, true, errors)
    } else {
      loop {
        let Some(OpenAst {
          encloser_or_operator_index,
          encloser_or_operator,
          ..
        }) = self.open_asts.last()
        else {
          unreachable!()
        };
        match encloser_or_operator {
          EncloserOrOperator::Encloser(encloser) => {
            errors.push(ParseError {
              kind: ParseErrorKind::EndOfTextWithOpenEncloser(
                encloser.opening_encloser_str().to_string(),
              ),
              pos: *encloser_or_operator_index
                ..encloser_or_operator_index
                  + encloser.opening_encloser_str().len(),
            });
          }
          EncloserOrOperator::Operator(operator) => {
            errors.push(ParseError {
              kind: ParseErrorKind::OperatorMissingRightArgument(
                operator.op_str().to_string(),
              ),
              pos: *encloser_or_operator_index
                ..encloser_or_operator_index + operator.op_str().len(),
            });
          }
        }
        close_ast_and_maybe_return!(self.text.len(), true)
      }
    }
  }
}
