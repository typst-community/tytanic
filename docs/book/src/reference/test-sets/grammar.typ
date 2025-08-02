#import "/book.typ": book-page
#show: book-page.with(title: "Grammar")

The exact grammar can be read from the source code at #link("https://github.com/typst-community/tytanic/blob/main/crates/tytanic-filter/src/ast/grammar.pest")[grammar.pest].
Because it is a functional language it consists only of expressions, no statements.

It supports
- groups for precedence (`(...)`),
- binary and unary operators (`and`, `not`, `!`, etc.),
- functions (`func(a, b, c)`),
- patterns (`r:^foo`, `r:"foo,?"`),
- and basic data types like strings (`"..."`, `'...'`) and numbers (`1`, `1_000`).

= Operators
The following operators are available:

#table(
  columns: 5,
  table.header[Type][Prec.][Name][Symbols][Explanation],
  table.hline(),

  [infix], [1], [union], [`|`, `or`],
  [Includes all tests which are in either the left OR right test set expression.],

  [infix], [1], [difference], [`~`, `diff`],
  [Includes all tests which are in the left but NOT in the right test set expression.],

  [infix], [2], [intersection], [`&`, `and`],
  [Includes all tests which are in both the left AND right test set expression.],

  [infix], [3], [symmetric difference], [`^`, `xor`],
  [Includes all tests which are in either the left OR right test set expression, but NOT in both.],

  [prefix], [4], [complement], [`!`, `not`],
  [Includes all tests which are NOT in the test set expression.],
)


Be aware of precedence when combining different operators, higher precedence means operators bind more strongly, e.g. `not a and b` is `(not a) and b`, not `not (a and b)` because `not` has a higher precedence than `and`.
Binary operators are left associative, e.g. `a ~ b ~ c` is `(a ~ b) ~ c`, not `a ~ (b ~ c)`.
When in doubt, use parentheses to force the precedence of expressions.
