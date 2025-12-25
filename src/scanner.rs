/// Scans for alternating semicolons and newlines.
struct Scanner<'a> {
  buffer: &'a [u8],
}
