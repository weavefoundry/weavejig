class K {
  int[] a1 = {
    100000000, 200000000, 300000000, 400000000, 500000000, 600000000, 700000000, 800000000,
    900000000
  };
  String[] a2 = {
    "aaaaaaaaaaaaaaa",
    "bbbbbbbbbbbbbbbbbbbb",
    "cccccccccccccccccccc",
    "dddddddddddddddddddd",
    "eeeeeeeeeeeeeeeeee"
  };
  Object[] a3 = {
    aaaaaaaaaaaaaaaaaaaaaa,
    bbbbbbbbbbbbbbbbbbbbbbbbb,
    ccccccccccccccccccccccc,
    dddddddddddddddddddddd,
    eeeeeeeeeeeeeee
  };
  Object[] a4 = {
    aaaaaaaaaaaaaaaaaaaaaa(),
    bbbbbbbbbbbbbbbbbbbbbbbbb,
    ccccccccccccccccccccccc,
    dddddddddddddddddddddd,
    eeeeeeeeeeeeeee
  };
  Object[] a5 = {
    Foo.AAAAAAAAAAAAAAAAAAA,
    Foo.BBBBBBBBBBBBBBBBBBBBBBBBBB,
    Foo.CCCCCCCCCCCCCCCCCCCCC,
    Foo.DDDDDDDDDDDDDDDDDDD,
    Foo.EEEEE
  };
  Object[] a6 = {
    -100000000,
    2222222222L,
    'c',
    true,
    null,
    400000000.0,
    0x55555555,
    600000000,
    700000000,
    800000000,
    900000000,
    1
  };
  Object[] a7 = {
    100000000 + 1,
    200000000,
    300000000,
    400000000,
    500000000,
    600000000,
    700000000,
    800000000,
    900000000
  };
  int[][] a8 = {
    {100000000, 200000000},
    {300000000, 400000000},
    {500000000, 600000000},
    {700000000, 800000000},
    {9, 9}
  };

  void m() {
    foo(
        100000000, 200000000, 300000000, 400000000, 500000000, 600000000, 700000000, 800000000,
        900000000);
    foo(
        "aaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbb",
        "cccccccccccccccccccc",
        "dddddddddddddddddddd",
        "eeeeeeeeeeeeeeeeee");
    foo(
        aaaaaaaaaaaaaaaaaaaaaa,
        bbbbbbbbbbbbbbbbbbbbbbbbb,
        ccccccccccccccccccccccc,
        dddddddddddddddddddddd,
        eeeeeeeeeeeeeee);
    foo(
        100000000,
        200000000,
        300000000,
        400000000,
        500000000,
        600000000,
        700000000,
        800000000,
        xxxxxxxxxx);
    foo(
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w');
    foo(
        100000000,
        "222222222222",
        300000000,
        400000000,
        500000000,
        600000000,
        700000000,
        800000000,
        true,
        null);
    foo(
        -100000000,
        -200000000,
        300000000,
        400000000,
        500000000,
        600000000,
        700000000,
        800000000,
        90000000);
    ImmutableSortedMap.of(
        "one", 1, "two", 2, "three", 3, "four", 4, "five", 5, "six", 6, "seven", 7, "eight", 8, "n",
        9);
    ImmutableSortedMap.of(
        "one",
        1,
        "two",
        2,
        "three",
        3,
        "four",
        4,
        "five",
        5,
        "six",
        6,
        "seven",
        7,
        "eight",
        8,
        "nine",
        9,
        "tennnnnnnnnnn",
        10);
    ImmutableSortedMap.of(
        "one",
        1,
        "two",
        2,
        "three",
        3,
        "four",
        4,
        "five",
        5,
        "six",
        6,
        "seven",
        7,
        "eight",
        8,
        "nine",
        9,
        "tennnnnnnnnnn",
        10,
        "e",
        11);
    foo(
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    foo(
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
        ccccc,
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
        ccccc);
    FileSystem fs =
        Jimfs.newFileSystem(
            Configuration.unix().toBuilder()
                .setSupportedFeatures(ObjectArrays.concat(SYMBOLIC_LINKS, supportedFeatures))
                .build());
    Configuration.unixxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx()
        .toBuilderrrrrrrrrrrrrrrrrrrrrrrrrrrrrr()
        .setSupportedFeatures(aaaaaaaaaaaaaaaaaaaaaa)
        .build();
    Configuration.unixxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx()
        .toBuilderrrrrrrrrrrrrrrrrrrrrrrrrrrrrr()
        .setSupportedFeatures(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    Configuration.unixxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx(a)
        .toBuilderrrrrrrrrrrrrrrrrrrrrrrrrrrrrr()
        .setSupportedFeatures(aaaaaaaaaaaaaaaaaaaaaa)
        .build();
    fooooooooooooooooooooooooo
        .unixxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx()
        .toBuilderrrrrrrrrrrrrrrrrrrrrrrrrrrrrr()
        .setSupportedFeatures(aaaaaaaaaaaaaaaaaa)
        .build();
    SpliteratorTester.of(() ->
            CollectSpliterators.flatMap(
                Arrays.spliterator(new String[] {"abc", "", "de", "f", "g", ""}), 7))
        .expect('a', 'b');
    foooooooooooooo(() ->
        CollectSpliterators.flatMap(
            Arrays.spliterator(new String[] {"abc", "", "de", "f", "g", ""}), 7, 888888888));
    foooooooooooooo(
        aaaaaa,
        () ->
            CollectSpliterators.flatMap(
                Arrays.spliterator(new String[] {"abc", "", "de", "f", "g", ""}), 7, 8888));
    foooooooooooooo(
        aaaaaa,
        x -> {
          return 1;
        });
    foooooooooooooo(
        x -> {
          return 1;
        },
        aaaaa);
    foooooooooooooo(
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
        x -> {
          return 1;
        });
    Object o =
        (LocalManualCacheeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee)
            CacheBuilder.newBuilderrrrrrrrrrrrrrrrrrrrrr(aaaaaaaa);
    Object o =
        (ClosingCallable<TestCloseable>)
            closer -> {
              return closeable3;
            };
    new Thread() {
      public void run() {}
    }.start();
    new Thread() {
      public void run() {}
    }.start().foo();
    x =
        new Thread() {
          public void run() {}
        };
  }
}
