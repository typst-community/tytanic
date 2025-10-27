#import  "/src/internal.typ": helper

#let template(title: none) = body => {
  assert.ne(title, none, message: "`title` is not optional")

  align(center + horizon, title)
  pagebreak()
  body
}
