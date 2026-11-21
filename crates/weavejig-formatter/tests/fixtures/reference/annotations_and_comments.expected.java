import foo.Bar;

class K {
  @SuppressWarnings({
    "serial", // we're overriding default serialization
    "TooManyParameters",
  })
  @Param({
    "3", "6", "11", "23", "45", "91", "181", "362", "724", "1448", "2896", "5793", "11585", "23170",
    "46341", "92682", "185364"
  })
  @Target({ElementType.METHOD, ElementType.TYPE})
  private int size;

  @VisibleForTesting
  static final double[] everySixteenthFactorial = {
    0x1.0p0, 0x1.30777758p44, 0x1.956ad0aae33a4p117, 0x1.ee69a78d72cb6p202
  };

  @GwtIncompatible @J2ktIncompatible
  private static final long serialVersionUID = 6588350623831699109L;

  @GwtIncompatible @J2ktIncompatible private static final long shortEnough = 6588350623831699109L;

  @Bar
  Object fieldddddddddddddddddddddddddddddddddddddddddddddddddd =
      new Objectttttttttttttttttttttttttttttttttt();

  char[] surrogates = {
    MAX_LOW_SURROGATE, MAX_HIGH_SURROGATE, MIN_LOW_SURROGATE, MIN_HIGH_SURROGATE,
  };

  enum E {
    A {
      void m() {}
    },
    B {},
    ;
    int x;
  }

  void m() {
    try (ObjectInputStream ois =
        new ObjectInputStream(new ByteArrayInputStream(baos.toByteArrayyyyyyyyyyyy()))) {}
    try (Reader a = open();
        Reader joinedReader =
            CharSource.concat(nCopies(100_000, CharSource.empty())).openStream()) {}
    assertEvents(
        "U1.00", "U1.00", "R0.00", "R0.00", "R0.00", "R0.00", // first request and burst
        "R0.20");
    Reflection.initialize(
        A.class, // Already initialized (above)
        B.class, C.class);
    foo(
        ImmutableSet.of(
            // DurationSpec.of(500, MILLISECONDS),
            DurationSpec.of(1, SECONDS), DurationSpec.of(1, DAYS)));
    return "vetoableChange".equals(method.getName())
            && "Hour".equals(((PropertyChangeEvent) args[0]).getPropertyNameeeeeeee())
        ? null
        : x;
    @Bar
    Object locallllllllllllllllllllllllllllllllllllllllllllllllllllll =
        new Objectttttttttttttttttttttttttttttttttt();
  }

  TestSuite computeMultimapGetTestSuite(
      FeatureSpecificTestSuiteBuilder<
              ?, ? extends OneSizeTestContainerGenerator<SetMultimap<K, V>, Entry<K, V>>>
          parentBuilder) {}
}
