class H {
  void m() {
    var a =
        someCollectionOfThings.stream().map(thing -> thing.getOtherThings()).stream()
            .filter(other -> other.isGood())
            .collect(toList());
    var b =
        someCollectionOfThings.getTheThings(withSomeArgument, andAnotherArgument).stream()
            .filter(other -> other.isGood())
            .collect(toList());
    var c =
        someCollectionOfThings
            .getTheThings(withSomeArgument, andAnotherArgument, andYetAnotherArgumentThatIsLong)
            .stream()
            .filter(other -> other.isGood())
            .collect(toList());
    var d =
        findAnnotatedMethods(
            testClass,
            ClassTemplateInvocationLifecycleMethod.class,
            HierarchyTraversalMode.TOP_DOWN)
            .stream();
    var e =
        things
            .getTheThings(
                withSomeArgument, andAnotherArgument, andYetAnotherArgumentThatIsLong, more)
            .stream()
            .filter(other -> other.isGood());
    var f =
        resources.stream() //
            .collect(groupingBy(Resource::getName)) //
            .values() //
            .stream() //
            .map(x -> x);
    var g =
        builderThing.toBuilder()
            .setSomething(someValueThatIsLong)
            .setSomethingElse(anotherValueThatIsLong)
            .toBuilder()
            .build();
  }
}
