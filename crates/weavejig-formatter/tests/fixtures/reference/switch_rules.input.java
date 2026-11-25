class S {
  void m(Object o, int k) {
    switch (result.getStatus()) {
      case ABORTED -> watcher.testAborted(extensionContext, result.getThrowable().orElse(null), andSomeMore);
      case FAILED -> watcher.testFailed(extensionContext);
      case SUCCESSFUL, OTHER -> {
        done();
      }
      case A_VERY_LONG_CONSTANT_NAME_NUMBER_ONE, A_VERY_LONG_CONSTANT_NAME_NUMBER_TWO, A_VERY_LONG_CONSTANT_NAME_NUMBER_THREE -> x();
      case B -> throw new IllegalStateException("some very long message that goes on and on and on and on and on");
      default -> {}
    }
    var v = switch (o) {
      case String s when s.length() > 10 && s.startsWith("some very long prefix that goes on and on") && other(s) -> 1;
      case Integer i -> concatStreams(prefixed(allEvents, it -> it.endsWith(":bar") ? testClassName : "test(TestReporter)"), prefixed(allEvents, it -> 1));
      case Point(int x, int y) -> x + y;
      case null, default -> 0;
    };
    int w = switch (k) { case 1: yield 2; default: yield 3; };
    return switch (k) {
      case 1 -> "one";
      default -> "many";
    };
  }
}
