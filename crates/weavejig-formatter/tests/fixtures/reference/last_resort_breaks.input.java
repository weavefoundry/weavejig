class M {
  CommandResult<?> run(String[] args, @SuppressWarnings("OptionalUsedAsFieldOrParameterType") Optional<OutputStreamConfig> outputStreamConfig) {}

  void a(@TestParameter({"IDENTITYIDENTITYIDENTITYIDENTITY", "UPPER_CAMEL_CASEUPPER_CAMEL_CASEUPPER_CAMEL_CASE", "UPPER_CASE_WITH_UNDERSCORES"}) FieldNamingPolicy fieldNamingPolicy, final int x) {}

  void b(final @Nullable VeryLongTypeNameThatGoesOnAndOnAndOnAndOn<WithSomeTypeArguments, AndMoreOfThem> parameterNameThatIsLong, int y) {}

  void c(@A @B(1) VeryLongTypeNameThatGoesOnAndOnAndOnAndOnAndOnAndOn<WithSomeTypeArguments, AndMoreOfThem> parameterNameThatIsLong) {}

  boolean d(Object o) {
    return RegularImmutableTable.forCellsAndSomeOtherLongMethodName(ImmutableSet.of(immutableCell('a', 1, "blah"))) instanceof SparseImmutableTable<?, ?, ?>;
  }

  boolean e(Object o) {
    return someObjectWithAVeryLongNameIndeedSoLong.andAFieldAlsoQuiteLong instanceof SparseImmutableTable<?, ?, ?> table && table.isEmpty();
  }

  void f() {
    stream.filter(uniqueId(lastSegmentType(ClassTemplateInvocationTestDescriptor.SEGMENT_TYPE_LONGER_NAME))::matches).map(TestDescriptor::getDisplayName);
    Function<String, String> f = SomeVeryLongClassNameThatGoesOnAndOnAndOnAndOn.WithANestedClassAlsoLong::someMethodReferenceName;
    x = Constants.DEFAULT_TEST_CLASS_INSTANCE_CONSTRUCTION_EXTENSION_CONTEXT_SCOPE_PROPERTY_NAME_AND_SOME_MORE_TEXT;
    call(Constants.DEFAULT_TEST_CLASS_INSTANCE_CONSTRUCTION_EXTENSION_CONTEXT_SCOPE_PROPERTY_NAME_AND_SOME, 2);
    var y = this.someFieldWithALongNameAndMore.anotherFieldWithALongNameAndMore.yetAnotherFieldWithALongName.end;
    var z = org.junit.platform.engine.support.hierarchical.SomeClassNameThatIsLong.SOME_CONSTANT_THAT_IS_LONG;
    Class<?> c = DefaultOrderAndExplicitOrderInstanceLevelExtensionRegistrationWithTestInstancePerClassLifecycleTestCase.class;
  }

  private static <FutureT extends AbstractFuture<@Nullable Integer> & SettableAbstractFuture<@Nullable Integer>> void assertFailed(FutureT future) {}

  private static <InElementT extends @Nullable Object, OutSpliteratorT extends Spliterator.OfPrimitive<OutElementT, OutConsumerT, OutSpliteratorT>> void g() {}

  private record CsvExceptionHandlingSpliterator<T>(Spliterator<T> delegate, CsvFileSource csvFileSource) implements Spliterator<T> {}

  private record Shorter<T>(Spliterator<T> delegate, CsvFileSource csvFileSource, int more) implements Spliterator<T>, Other {}
}
