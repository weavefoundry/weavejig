class K {
  byte[] rows4 = {
    (byte) 0xFF,
    (byte) 0xEE,
    (byte) 0xDD,
    (byte) 0xCC,
    (byte) 0xBB,
    (byte) 0xAA,
    (byte) 0x99,
    (byte) 0x88
  };
  byte[] rows3 = {(byte) 0xFF, (byte) 0xEE, (byte) 0xDD, (byte) 0xBB, (byte) 0xAA, (byte) 0x99};
  byte[] rowsUneven = {
    (byte) 0xFF, (byte) 0xEE, (byte) 0xDD, (byte) 0xBB, (byte) 0xAA, (byte) 0x99, (byte) 0x88
  };
  byte[] rowsLast = {
    (byte) 0xFF, (byte) 0xEE, (byte) 0xDD, (byte) 0xBB, (byte) 0xAA, (byte) 0x99, (byte) 0x88
  };
  byte[] rows2 = {(byte) 0xFF, (byte) 0xEE, (byte) 0xBB, (byte) 0xAA};
  byte[] rows1 = {(byte) 0xFF, (byte) 0xEE};
  byte[] oneRow = {
    (byte) 0xFF,
    (byte) 0xEE,
    (byte) 0xDD,
    (byte) 0xCC,
    (byte) 0xBB,
    (byte) 0xAA,
    (byte) 0x99,
    (byte) 0x88,
    (byte) 0x88
  };
  String[][] pairs = {
    {"a", "A"},
    {"ab", "Ab"},
    {"AB", "AB"},
    {"_a", "_A"},
    {"_ab", "_Ab"},
    {"__", "__"},
    {"_1", "_1"},
    {"x", "y"}
  };

  void m() {
    foo(
        (byte) 0xFF,
        (byte) 0xEE,
        (byte) 0xDD,
        (byte) 0xCC,
        (byte) 0xBB,
        (byte) 0xAA,
        (byte) 0x99,
        (byte) 0x88);
    foo(
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    sort(
        entries, // Entries up to size are not null
        0,
        size,
        Ordering.from(valueComparator).onResultOf(Entry::getValue));
    sort(
        // leading comment
        entries, 0, size, Ordering.from(valueComparator).onResultOf(Entry::getValue));
    sort(
        entries,
        0,
        // middle comment
        size,
        Ordering.from(valueComparator).onResultOf(Entry::getValue));
    f(
        i1, i2, i3, // one
        s1, s2 // two
        );
    f(
        i1,
        i2,
        // own line
        i3,
        s1);
    g(
        /* prefix= */ prefix,
        /* suffix= */ null,
        /* directory= */ null /* defaults to java.io.tmpdir */);
    for (Iterator<Entry<K, V>> unfilteredItr = multimap.unfiltered().entries().iterator();
        unfilteredItr.hasNext(); ) {}
    for (; ; ) {}
    for (int i = 0; ; i++) {}
  }
}
