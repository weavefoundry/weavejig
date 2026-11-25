class L {
  Object[] a1 = {{1, 2}, {3, 4}};
  Object[] a2 = {new String[] {"test1", "test2"}, new String[] {"test3", "test4"}};
  Object[] a3 = {new String[] {"test1", "test2"}};
  Object[] a4 = {
    {1, 2},
    {3, 4}
  };
  Object[] a5 = {
    new String[] {"test1", "test2"},
    new String[] {"test3", "test4"}
  };
  Object[] a6 = {new String[] {"test1", "test2"}};
  Object[] a7 = {f(1), f(2)};
  Object[] a8 = {
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "ccccccccccccccccccccccccccccccc"
  };
  Object[] a9 = {aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb};
  Object x =
      new LookupTranslator(
          new String[][] {
            {"\"", "\\\""},
            {"\\", "\\\\"},
            {"/", "\\/"}
          });
  Object y = new LookupTranslator(new String[][] {{"\"", "\\\""}, {"\\", "\\\\"}, {"/", "\\/"}});
  Object z =
      new byte[] {
        0x00, 0x7F, (byte) 0xFF,
        0x3F, 0x7F, 0x3F,
        0x3F, 0x7F, 0x3F
      };

  @A({"apple,         1", "banana,        2"})
  @B({"deprecation", "InlineMeInliner"})
  @C({
    "deprecation",
    "InlineMeInliner",
  })
  @D({A.class, B.class})
  @E(
      value = {A.class, B.class},
      other = 1)
  int w;
}
