#let helper(body) = {
  assert.eq(type(body), str, message: "`body` must be of type str")

  [Helper: #body]
}
