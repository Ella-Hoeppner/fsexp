use std::{fmt::Debug, ops::Range};
use take_mut::take;

use crate::{
  Ast, Encloser, Operator, ParseError, Parser, SyntaxTree,
  ast::{InvalidTreePath, Sexp},
  syntax::{
    ContainsEncloserOrOperator, ContextId, EncloserOrOperator, IdStr, Syntax,
  },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct DocumentPosition {
  pub span: Range<usize>,
  pub path: Vec<usize>,
}
impl DocumentPosition {
  pub fn new(span: Range<usize>, path: Vec<usize>) -> Self {
    Self { span, path }
  }
  pub fn start(&self) -> usize {
    self.span.start
  }
  pub fn end(&self) -> usize {
    self.span.end
  }
}

pub type DocumentSyntaxTree<E, O> =
  Ast<DocumentPosition, (DocumentPosition, EncloserOrOperator<E, O>)>;

impl<E: Encloser, O: Operator> ContainsEncloserOrOperator<E, O>
  for (DocumentPosition, EncloserOrOperator<E, O>)
{
  fn get_encloser_or_operator(&self) -> &EncloserOrOperator<E, O> {
    &self.1
  }

  fn into_encloser_or_operator(self) -> EncloserOrOperator<E, O> {
    self.1
  }
}

impl<E: Encloser, O: Operator> DocumentSyntaxTree<E, O> {
  pub fn from_raw_ast_with_path<
    L: Clone + PartialEq + Debug,
    I: Clone + PartialEq + Debug + ContainsEncloserOrOperator<E, O>,
  >(
    ast: Ast<L, I>,
    path: Vec<usize>,
  ) -> Self {
    match ast {
      Ast::Leaf(_, leaf) => {
        Self::Leaf(DocumentPosition { span: 0..0, path }, leaf)
      }
      Ast::Inner(data, children) => {
        let translated_children = children
          .into_iter()
          .enumerate()
          .map(|(i, child)| {
            Self::from_raw_ast_with_path(child, {
              let mut new_path = path.clone();
              new_path.push(i);
              new_path
            })
          })
          .collect();
        Self::Inner(
          (
            DocumentPosition { span: 0..0, path },
            data.into_encloser_or_operator(),
          ),
          translated_children,
        )
      }
    }
  }
  pub fn from_raw_ast<
    L: Clone + PartialEq + Debug,
    I: Clone + PartialEq + Debug + ContainsEncloserOrOperator<E, O>,
  >(
    ast: Ast<L, I>,
  ) -> Self {
    Self::from_raw_ast_with_path(ast, vec![])
  }
  pub fn position(&self) -> &DocumentPosition {
    match self {
      Ast::Leaf(position, _) => position,
      Ast::Inner((position, _), _) => position,
    }
  }
  pub fn encloses(&self, selection: &Range<usize>) -> bool {
    self.position().start() <= selection.start
      && self.position().end() >= selection.end
  }
  pub fn enclosed_by(&self, selection: &Range<usize>) -> bool {
    self.position().start() >= selection.start
      && self.position().end() <= selection.end
  }
  pub fn calculate_paths(self, parent_path: Vec<usize>) -> Self {
    use Ast::*;
    match self {
      Leaf(DocumentPosition { span, .. }, leaf) => Leaf(
        DocumentPosition {
          span,
          path: parent_path,
        },
        leaf,
      ),
      Inner(
        (DocumentPosition { span, .. }, encloser_or_operator),
        children,
      ) => {
        let children = children
          .into_iter()
          .enumerate()
          .map(|(i, child)| {
            let mut sub_path = parent_path.clone();
            sub_path.push(i);
            child.calculate_paths(sub_path)
          })
          .collect();
        Inner(
          (
            DocumentPosition {
              span,
              path: parent_path,
            },
            encloser_or_operator,
          ),
          children,
        )
      }
    }
  }
  pub fn filter_comments<S: Syntax<E = E, O = O>>(
    self,
    syntax: &S,
  ) -> Option<Self> {
    match self {
      Ast::Inner((span, encloser_or_operator), children) => (!syntax
        .encloser_or_operator_context(&encloser_or_operator)
        .map(|ctx| ctx.is_comment())
        .unwrap_or(false))
      .then(|| {
        Ast::Inner(
          (span, encloser_or_operator),
          children
            .into_iter()
            .filter_map(|child| child.filter_comments(syntax))
            .collect(),
        )
      }),
      leaf => Some(leaf),
    }
  }
  fn replace_subtree_inner(
    &mut self,
    mut path: impl Iterator<Item = usize>,
    mut replacement: Self,
  ) {
    match path.next() {
      Some(i) => match self {
        Ast::Leaf(_, _) => {
          panic!("replace_subtree_inner encountered a leaf along provided path")
        }
        Ast::Inner(_, children) => {
          if i < children.len() {
            children[i].replace_subtree_inner(path, replacement);
          } else {
            panic!("replace_subtree_inner tried to go to an invalid index")
          }
        }
      },
      None => std::mem::swap(self, &mut replacement),
    }
  }
  pub fn replace_subtree(
    &mut self,
    path: Vec<usize>,
    path_prefix: Vec<usize>,
    replacement: Self,
  ) {
    let modify_pos = |mut pos: DocumentPosition| {
      pos.path = path_prefix
        .iter()
        .cloned()
        .chain(path.iter().copied().chain(pos.path))
        .collect();
      pos
    };
    let modified_replacement = replacement.map_owned(
      &|pos, leaf| (modify_pos(pos), leaf),
      &|(pos, encloser_or_operator)| (modify_pos(pos), encloser_or_operator),
    );
    self.replace_subtree_inner(path.into_iter(), modified_replacement)
  }
  fn gather_annotations<Annotation: Clone, WalkState>(
    &self,
    walk_state: &WalkState,
    annotation_log: &mut Vec<(Range<usize>, Annotation)>,
    leaf_annotator: &impl Fn(&str, &WalkState) -> Option<Annotation>,
    encloser_annotator: &impl Fn(&E, &WalkState) -> (WalkState, Option<Annotation>),
    operator_annotator: &impl Fn(&O, &WalkState) -> (WalkState, Option<Annotation>),
  ) {
    match self {
      Ast::Leaf(position, leaf) => {
        if let Some(annotation) = leaf_annotator(leaf, walk_state) {
          annotation_log.push((position.span.clone(), annotation))
        }
      }
      Ast::Inner((position, encloser_or_operator), children) => {
        let inner_walk_state = match encloser_or_operator {
          EncloserOrOperator::Encloser(encloser) => {
            let (inner_walk_state, maybe_annotation) =
              encloser_annotator(encloser, walk_state);
            if let Some(annotation) = maybe_annotation {
              let start = position.span.start;
              let end = position.span.end;
              annotation_log.push((
                (start..start + encloser.opening_encloser_str().len()),
                annotation.clone(),
              ));
              annotation_log.push((
                (end - encloser.closing_encloser_str().len()..end),
                annotation,
              ));
            }
            inner_walk_state
          }
          EncloserOrOperator::Operator(operator) => {
            let (inner_walk_state, maybe_annotation) =
              operator_annotator(operator, walk_state);
            if let Some(annotation) = maybe_annotation {
              let start = if operator.left_args() == 0 {
                position.start()
              } else {
                children[operator.left_args() - 1].position().end()
              };
              let end = if operator.right_args() == 0
                || operator.left_args() >= children.len()
              {
                position.end()
              } else {
                children[operator.left_args()].position().start()
              };
              annotation_log.push((start..end, annotation));
            }
            inner_walk_state
          }
        };
        for child in children.iter() {
          child.gather_annotations(
            &inner_walk_state,
            annotation_log,
            leaf_annotator,
            encloser_annotator,
            operator_annotator,
          );
        }
      }
    }
  }
}

impl<E: Encloser, O: Operator> From<DocumentSyntaxTree<E, O>>
  for SyntaxTree<E, O>
{
  fn from(tree: DocumentSyntaxTree<E, O>) -> Self {
    match tree {
      DocumentSyntaxTree::Leaf(_, leaf) => SyntaxTree::Leaf((), leaf),
      DocumentSyntaxTree::Inner((_, encloser_or_opener), subtrees) => {
        SyntaxTree::Inner(encloser_or_opener, {
          subtrees.into_iter().map(SyntaxTree::from).collect()
        })
      }
    }
  }
}

impl<E: Encloser + IdStr, O: Operator + IdStr> From<DocumentSyntaxTree<E, O>>
  for Sexp
{
  fn from(tree: DocumentSyntaxTree<E, O>) -> Self {
    SyntaxTree::from(tree).into()
  }
}

#[derive(Debug, Clone)]
pub struct Document<S: Syntax> {
  newline_indeces: Vec<usize>,
  pub syntax: S,
  pub syntax_trees: Vec<DocumentSyntaxTree<S::E, S::O>>,
  pub parsing_failures: Vec<ParseError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidDocumentIndex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidDocumentCharPos;

impl<'t, S: Syntax> From<Parser<'t, S>> for Document<S> {
  fn from(mut parser: Parser<'t, S>) -> Self {
    let newline_indeces =
      parser.text.match_indices('\n').map(|(i, _)| i).collect();
    let (mut syntax_trees, parsing_failures) = parser.read_all();
    syntax_trees = syntax_trees
      .into_iter()
      .enumerate()
      .map(|(i, tree)| tree.calculate_paths(vec![i]))
      .collect();
    Self {
      newline_indeces,
      syntax_trees,
      parsing_failures,
      syntax: parser.syntax,
    }
  }
}

impl<S: Syntax> Document<S> {
  pub fn from_text_with_syntax(syntax: S, text: &str) -> Self {
    Parser::new(syntax, text).into()
  }
  pub fn get_subtree(
    &self,
    path: &[usize],
  ) -> Result<&DocumentSyntaxTree<S::E, S::O>, InvalidTreePath> {
    let mut path_iter = path.iter().copied();
    if let Some(top_level_tree_index) = path_iter.next()
      && let Some(top_level_tree) = self.syntax_trees.get(top_level_tree_index)
    {
      top_level_tree.get_subtree_inner(path_iter)
    } else {
      Err(InvalidTreePath)
    }
  }
  pub fn get_subtree_text<'t>(
    &self,
    path: &[usize],
    text: &'t str,
  ) -> Result<&'t str, InvalidTreePath> {
    let pos = self.get_subtree(path)?.position();
    Ok(&text[pos.start()..pos.end()])
  }
  pub fn innermost_predicate_path(
    &self,
    predicate: &impl Fn(&DocumentSyntaxTree<S::E, S::O>) -> bool,
  ) -> Option<Vec<usize>> {
    self
      .syntax_trees
      .iter()
      .enumerate()
      .find_map(|(i, syntax_tree)| {
        syntax_tree.innermost_predicate_reverse_path(predicate).map(
          |mut reverse_path| {
            reverse_path.push(i);
            reverse_path.reverse();
            reverse_path
          },
        )
      })
  }
  pub fn innermost_enclosing_path(
    &self,
    selection: &Range<usize>,
  ) -> Vec<usize> {
    self
      .innermost_predicate_path(&|tree| tree.encloses(selection))
      .unwrap_or(vec![])
  }
  pub fn expand_selection(
    &self,
    selection: &Range<usize>,
  ) -> Option<Range<usize>> {
    let path = self.innermost_enclosing_path(selection);
    if path.is_empty() {
      None
    } else {
      let tree = self.get_subtree(&path).unwrap();
      if tree.position().span == *selection {
        if path.len() == 1 {
          if self.syntax_trees.len() < 2 {
            None
          } else {
            Some(
              self.syntax_trees.first().unwrap().position().start()
                ..self.syntax_trees.last().unwrap().position().end(),
            )
          }
        } else {
          let mut path = path;
          path.pop();
          Some(self.get_subtree(&path).unwrap().position().span.clone())
        }
      } else {
        Some(tree.position().span.clone())
      }
    }
  }
  pub fn move_cursor_to_start(&self, selection: &Range<usize>) -> usize {
    if selection.start != selection.end {
      return selection.start;
    }
    let cursor = selection.start;
    let mut enclosing_path = self.innermost_enclosing_path(selection);
    if enclosing_path.is_empty() {
      if let Some(preceding_tree_index) = self
        .syntax_trees
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, tree)| tree.position().end() < cursor)
        .map(|(i, _)| i)
        .next()
      {
        enclosing_path.push(preceding_tree_index);
      }
    } else {
      let enclosing_subtree = self.get_subtree(&enclosing_path).unwrap();
      let start_of_enclosing = enclosing_subtree.position().start();
      let end_of_enclosing = enclosing_subtree.position().end();
      if cursor == start_of_enclosing {
        if enclosing_path.is_empty() {
          return cursor;
        } else if enclosing_path.last().unwrap() == &0 {
          enclosing_path.pop();
        } else {
          let last_index = enclosing_path.len() - 1;
          enclosing_path[last_index] -= 1;
        }
      } else if cursor != end_of_enclosing {
        return start_of_enclosing;
      }
    }
    if enclosing_path.is_empty() {
      cursor
    } else {
      self
        .get_subtree(&enclosing_path)
        .unwrap()
        .position()
        .start()
    }
  }
  pub fn move_cursor_to_end(&self, selection: &Range<usize>) -> usize {
    if selection.start != selection.end {
      return selection.end;
    }
    let cursor = selection.end;
    let mut enclosing_path = self.innermost_enclosing_path(selection);
    if enclosing_path.is_empty() {
      if let Some(preceding_tree_index) = self
        .syntax_trees
        .iter()
        .enumerate()
        .filter(|(_, tree)| tree.position().start() > cursor)
        .map(|(i, _)| i)
        .next()
      {
        enclosing_path.push(preceding_tree_index);
      }
    } else if cursor
      == self.get_subtree(&enclosing_path).unwrap().position().end()
    {
      loop {
        match enclosing_path.len() {
          0 => {
            return cursor;
          }
          1 => {
            if enclosing_path[0] < self.syntax_trees.len() - 1 {
              enclosing_path[0] += 1;
            }
          }
          path_length => {
            let parent_subtree = self
              .get_subtree(&enclosing_path[0..path_length - 1])
              .unwrap();
            if let DocumentSyntaxTree::Inner(
              (_, EncloserOrOperator::Operator(_)),
              _,
            ) = parent_subtree
              && parent_subtree.position().end() == cursor
            {
              enclosing_path.pop();
              continue;
            }
            let parent = self
              .get_subtree(&enclosing_path[0..path_length - 1])
              .unwrap();
            let sibling_count = if let Ast::Inner(_, siblings) = parent {
              siblings.len()
            } else {
              unreachable!("uh oh")
            };
            if *enclosing_path.last().unwrap() == sibling_count - 1 {
              enclosing_path.pop();
            } else {
              enclosing_path[path_length - 1] += 1;
            }
          }
        }
        break;
      }
    }
    if enclosing_path.is_empty() {
      cursor
    } else {
      self.get_subtree(&enclosing_path).unwrap().position().end()
    }
  }
  pub fn row_and_col_to_index(
    &self,
    row: usize,
    col: usize,
    text: &str,
  ) -> Result<usize, InvalidDocumentCharPos> {
    if row == 0 {
      if (self.newline_indeces.is_empty() && col <= text.len())
        || (!self.newline_indeces.is_empty() && col <= self.newline_indeces[0])
      {
        Ok(col)
      } else {
        Err(InvalidDocumentCharPos)
      }
    } else if row == self.newline_indeces.len() {
      let last_line_start = self.newline_indeces.last().unwrap();
      let index = last_line_start + 1 + col;
      if index <= text.len() {
        Ok(index)
      } else {
        Err(InvalidDocumentCharPos)
      }
    } else if row < self.newline_indeces.len() {
      let previous_line_start = self.newline_indeces[row - 1];
      let line_length = self.newline_indeces[row] - previous_line_start;
      if col < line_length {
        Ok(previous_line_start + 1 + col)
      } else {
        Err(InvalidDocumentCharPos)
      }
    } else {
      Err(InvalidDocumentCharPos)
    }
  }
  pub fn index_to_row_and_col(
    &self,
    index: usize,
    text: &str,
  ) -> Result<(usize, usize), InvalidDocumentIndex> {
    if index <= text.len() {
      Ok(
        self
          .newline_indeces
          .iter()
          .copied()
          .enumerate()
          .rev()
          .find_map(|(line_index, previous_newline_index)| {
            (previous_newline_index < index)
              .then(|| (line_index + 1, index - previous_newline_index - 1))
          })
          .unwrap_or((0, index)),
      )
    } else {
      Err(InvalidDocumentIndex)
    }
  }
  pub fn strip_comments(&mut self) {
    take(&mut self.syntax_trees, |trees| {
      trees
        .into_iter()
        .filter_map(|tree| tree.filter_comments(&self.syntax))
        .collect()
    })
  }
  pub fn map_trees(
    mut self,
    f: impl Fn(DocumentSyntaxTree<S::E, S::O>) -> DocumentSyntaxTree<S::E, S::O>,
  ) -> Self {
    take(&mut self.syntax_trees, |trees| {
      trees.into_iter().map(f).collect()
    });
    self
  }
  pub fn replace_leaves(self, leaf: &str, replacement: &str) -> Self {
    self.map_trees(|ast| {
      ast.map(
        &|data, l| {
          (
            data.clone(),
            if l == leaf {
              replacement.to_string()
            } else {
              l.to_string()
            },
          )
        },
        &|x| x.clone(),
      )
    })
  }
  pub fn gather_annotations<Annotation: Clone, WalkState>(
    &self,
    root_walk_state: WalkState,
    leaf_annotator: &impl Fn(&str, &WalkState) -> Option<Annotation>,
    encloser_annotator: &impl Fn(
      &S::E,
      &WalkState,
    ) -> (WalkState, Option<Annotation>),
    operator_annotator: &impl Fn(
      &S::O,
      &WalkState,
    ) -> (WalkState, Option<Annotation>),
  ) -> Vec<(Range<usize>, Annotation)> {
    let mut annotation_log = vec![];
    for tree in self.syntax_trees.iter() {
      tree.gather_annotations(
        &root_walk_state,
        &mut annotation_log,
        leaf_annotator,
        encloser_annotator,
        operator_annotator,
      );
    }
    annotation_log
  }
  pub fn get_line<'t>(&self, line_index: usize, text: &'t str) -> &'t str {
    &text[if line_index == 0 {
      0..self.newline_indeces.first().copied().unwrap_or(text.len())
    } else {
      self
        .newline_indeces
        .get(line_index - 1)
        .copied()
        .map(|i| i + 1)
        .unwrap_or(0)
        ..self
          .newline_indeces
          .get(line_index)
          .copied()
          .unwrap_or(text.len())
    }]
  }
  pub fn describe_document_position(
    &self,
    pos: Range<usize>,
    text: &str,
  ) -> String {
    let start = pos.start;
    let end = pos.end;
    let (start_row, start_col) =
      self.index_to_row_and_col(start, text).unwrap();
    let (end_row, end_col) = self.index_to_row_and_col(end, text).unwrap();
    let line_indices = (if start_row > 0 {
      start_row - 1
    } else {
      start_row
    })..=end_row;
    let mut line_names = line_indices
      .clone()
      .map(|i| format!("{}", i + 1))
      .collect::<Vec<String>>();
    let max_line_name_length =
      line_names.iter().map(|n| n.len()).max().unwrap();
    for n in line_names.iter_mut() {
      while n.len() < max_line_name_length {
        *n += " ";
      }
    }
    let mut lines = line_names
      .into_iter()
      .zip(line_indices)
      .map(|(name, i)| format!("{name} | {}", self.get_line(i, text)))
      .collect::<Vec<String>>()
      .join("\n");
    let (start_col, end_col) = (start_col.min(end_col), start_col.max(end_col));
    lines += "\n";
    lines += &" ".repeat(max_line_name_length + 3 + start_col);
    lines += &"^".repeat(end_col - start_col);
    lines
  }
}
