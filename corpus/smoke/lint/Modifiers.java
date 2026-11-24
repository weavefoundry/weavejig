package com.example.smoke.lint;

public abstract class Modifiers {
  final static int A = 1;
  static public void b() {}
  public @Deprecated static int c;
  public @Deprecated void d() {}
  public @SuppressWarnings("x") String e() {
    return "";
  }
  synchronized static final private void f() {}
  @Deprecated public static int g;
  abstract protected void h();
  final private transient volatile int i = 1;

  public @interface Marker {}

  public @Deprecated static class Inner {
    public Inner() {}
  }

  static @Deprecated class Package {
    public Package() {}

    private final void locked() {}
  }

  interface Shape {
    public static final int SIDES = 4;

    public abstract double area();

    public static class Square {}

    static enum Kind { ROUND }

    abstract interface Nested {}
  }

  enum Level {
    LOW;

    private Level() {}

    static final void all() {}
  }

  static enum Other { ONE }

  static interface Callback {
    void call(final int code);
  }

  final record Pair(int a, int b) {
    static record Inner() {}
  }

  strictfp double compute() {
    try (final java.io.StringReader reader = new java.io.StringReader("")) {
      return 1.0;
    }
  }

  Object anonymous = new Object() {
    final void run() {}
  };
}
