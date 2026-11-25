class L {
  int[] a = {
    1, 2, 3,
  };
  int[] b = {
    1, 2, 3,
  };
  int[] c = {
    1, 2, 3,
  };
  String[] d = {
    "deprecation", "InlineMeInliner",
  };
  String[] e = {
    "deprecation", "InlineMeInliner",
  };
  String[] f = {
    "deprecation",
    "InlineMeInliner",
    "abcdefghijklmnopqrstuvwxyz",
    "abcdefghijklmnopqrstuvwxyz",
    "abcdefghijklmnopqrstuvwxyz",
  };
  int[] g = {
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34,
  };

  @A({
    "deprecation",
    "InlineMeInliner",
  })
  @B({
    1, 2,
  })
  @C(
      x = {
        1, 2,
      },
      y = 3)
  int h;

  Object[] i = {
    {1, 2}, {3, 4},
  };
  Object[] j = {
    f(1), g(2),
  };
  Object[] k = {
    aaaaaaaaaa, bbbbbbbbb, 1,
  };
}
