package com.example.smoke.lint;

import java.util.ArrayList;
import java.util.List;

public class Cleanup implements Cloneable {
  private int count;
  private static int total;
  private List<String> names = new ArrayList<>();
  public int exposed;
  protected int inherited;
  int packaged;
  public static final int LIMIT = 3;
  public static int counter;
  public final int fixed = 4;

  // TODO: remove this class
  /* A block comment.
   * TODO: with a second line */
  /** Javadoc without the marker: TODO later. */
  public Cleanup(int count, List<String> names) {
    this.count = count;
    this.names = names;
  }

  Cleanup() {}

  protected Cleanup(String unused) {
    // deliberately empty
  }

  void setCount(int count) {
    int total = count;
    this.count = total;
  }

  static void reset(int count, int total) {
    Cleanup.total = total + count;
  }

  void lambdas() {
    Runnable r = () -> {
      int count = 1;
    };
    java.util.function.IntUnaryOperator f = count -> count + 1;
    for (int packaged = 0; packaged < 2; packaged++) {}
    try {
      r.run();
    } catch (RuntimeException exposed) {
      throw exposed;
    }
  }

  @Override
  protected void finalize() {
    count = 0;
  }

  @Override
  public Cleanup clone() {
    return new Cleanup();
  }

  void empty() {}

  void commented() {
    // nothing to do
  }

  void tooMany(int a, int b, int c, int d, int e, int f, int g, int h) {}

  int magic(int n) {
    int size = 42;
    final int width = 80;
    int[] sizes = {1, 2, 3, 100};
    long big = 0xFFFF_FFFFL * 7;
    if (n > 10 || n == -1 || n == 0x02) {
      return n * 3 + size;
    }
    assert n != 99 : "ninety-nine";
    return width + sizes.length;
  }

  int assigns(int a, int b, java.io.Reader in) throws java.io.IOException {
    int c = a = b;
    while ((c = in.read()) != -1) {
      b += c;
    }
    if ((a = b) > 0) {
      b = a;
    }
    a = b = c;
    return c;
  }

  static class Nested {
    private Nested() {}

    void finalize(int code) {}

    Object clone(int depth) {
      return null;
    }
  }

  static class Good implements Cloneable {
    @Override
    protected void finalize() throws Throwable {
      super.finalize();
    }

    @Override
    public Good clone() throws CloneNotSupportedException {
      return (Good) super.clone();
    }
  }

  private static class Hidden {}

  private static class Extended {}

  static class Extension extends Extended {}
}
