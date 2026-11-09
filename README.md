# fsexp

**F**lexible **S**-**Exp**ression parser, a customizable parser for lisp-like languages with support for a bit of extra syntactic sugar. Created mainly for use in [easl](https://github.com/Ella-Hoeppner/easl), but it's general enough to be useful for other projects.

## Explanation

### Enclosers and Operators

Fsexp grammars are defined mainly in terms of *enclosers* and *operators*.

An encloser is a set of two symbols, e.g. `(` and `)` or `{` and `}`, that get treated as a set of brackets that can enclose other expressions. Traditional s-expression syntax is basically just the fsexp grammar that has a single encloser consisting of `(` and `)`, and no operators. However, any pair of symbols (including multi-character symbols) can be used as enclosers in fsexp, making it very easy to parse lisp-like languages that use multiple different types of brackets with different semantic meanings, such as Clojure.

An operator is a special symbol that captures some number of expressions to it's left, and some other number of expressions to it's right. For instance, the quote special form `'` that many lisps use can be expressed as an operator that consumes zero arguments to it's left, and one argument to it's right. Infix operators, e.g. `:` for ascribing a type to an expression, can also be expressed as operators that consume one argument to their left and one to their right.

Operators may consume more than one expression to their left and their right. This doesn't come up too often, but can be useful for certain language constructs. For instance, Clojure uses the `^` symbol to ascribe "metadata" to values, with a syntax like `^my-metadata my-value`. In this case the `^` be seen as an operator that consumes zero arguments to it's left and two to it's right.

The fact that fsexp operators can consume expressions to the left as well as to the right, and therefore allows for infix operators, does represent a potentially significant divergence from traditional lispy syntax. The intention with this capability is that it should only be used for syntactic elements that aren't likely to cause ambiguity or confusion when reading code, and even then should only be used sparingly. As such, fsexp doesn't provide any way to specify the precedence for different operators - everything is just parsed left-to-right with equal precedence. Fsexp isn't intended to be a general-purpose parser for any kind of programming language, it's intended for lisp-like languages with a few additional syntactic elements beyond basic s-expressions.

### Contexts

A full fsexp grammar consists of a set of *contexts*, where each context can have it's own set of enclosers and operators. Each encloser or operator encountered while parsing may cause a switch to a different context for it's child expressions.

For instance, say that you wanted a grammar for s-expressions, but with support for arbitrary strings enclosed by `"`s. So all of the following expressions should parse successfully:

```
(a b "c")

(f "this is just one long expression, not multiple")

(f (g ")))"))
```

To start with, we might create a grammar that consists of two enclosers, one consisting of `(` and `)`, and another consisting of `"` and `"` (fsexp has no problem with the same character being used as both an opener and closer for an encloser). However, such a simple grammar would fail on the last example above, because first `)` inside the expression `")))"` would be treated as a closer that should match up to some preceeding corresponding `(` character, and therefore parsing would fail. To parse this successfully, we need to have two different contexts: a default context that includes the `(`/`)` and `"`/`"` enclosers, and a secondary context that contains no enclosers. This secondary context is for use inside a string, and will therefore be activated whenever the `"`/`"` encloser is encountered. When in this context, `)` is just treated as a normal character since the `(`/`)` encloser doesn't exist in this context. With this two-context setup, all the above examples parse properly.

Each context may also have it's own definition for which characters count as whitespace, for the purpose of deciding where one token ends and the next beings. In most contexts you'll just want spaces, tabs, and newlines to be treated as whitespace, but inside strings, for instance, you'll probably want the entire sequence of characters between the `"`s to be treated as a single token. To accomplish this you can simply declare that, inside the string context, spaces, tabs, and newlines are just normal characters, rather than being treated as "whitespace". This means that in the second example expression above, the inside of the `"this is just one long expression, not multiple"` expression is just one big token, rather than 8 separate tokens.

The [`sexp_with_strings.rs` example](https://github.com/Ella-Hoeppner/fsexp/tree/main/src/examples) contains an implemention for this grammar, with its two separate contexts and the `(`/`)` and `"`/`"` enclosers.

Aside from being useful for strings, this support for multiple contexts also makes it easy to handle line comments and block comments. For instance, many lisps use the `;` character to denote a single-line comment. In fsexp, this can be accomplished by defining an encloser consisting of `;` as an opener and the newline character `\n` a closer (along with a special context activated by this encloser that disables all other enclosers/operators inside, as with the string example).

The [`pseudo_clj.rs` example](https://github.com/Ella-Hoeppner/fsexp/tree/main/src/examples) provides a sophisticated example of an fsexp grammar, supporting most of Clojure's syntactic features.
