package com.example.smoke;

public class Expressions {
  static final long BIG = 0xFFFF_FFFFL;
  static final double D = 1e-9 + .5 + 1. + 0x1.8p3 + 3f;
  static final char[] CHARS = {'a', '\n', '\'', 'A'};
  static final int[][] GRID = {{1, 2}, {3, 4}, {}};
  static final String TEXT = """
      Hello,
        "world" \
      done
      """;

  int arithmetic(int a, int b) {
    int c = a + b * 2 - (a / b) % 3;
    c += a;
    c -= b;
    c *= 2;
    c /= 3;
    c %= 5;
    c <<= 1;
    c >>= 1;
    c >>>= 1;
    c &= 0xF;
    c |= 0b1010;
    c ^= 017;
    c = -c + +a - ~b;
    c = a++ + ++b - a-- - --b;
    return a > b ? a : b < c ? b : c;
  }

  boolean logic(Object o, int[] xs) {
    boolean b = !(o == null) && xs.length != 0 || xs[0] >= 1 && xs[xs.length - 1] <= 2;
    if (o instanceof String s && !s.isEmpty()) {
      return s.length() > 1 ^ b;
    }
    if (o instanceof final Integer i) {
      return i > 0;
    }
    return o instanceof Number | b & true;
  }

  String describe(Object o, int day) {
    String name =
        switch (day) {
          case 1, 7 -> "weekend";
          case 2, 3, 4, 5, 6 -> {
            String s = "week";
            yield s + "day";
          }
          default -> throw new IllegalArgumentException("bad day: " + day);
        };
    int size =
        switch (name) {
          case "weekend":
            yield 2;
          default:
            yield 5;
        };
    Object created = new Object() {
      @Override
      public String toString() {
        return name + size;
      }
    };
    int[] array = new int[size];
    int[][] matrix = new int[size][];
    String[] strings = new String[] {"a", "b"};
    Inner inner = this.new Inner();
    return created.toString() + array.length + matrix[0] + strings[1] + inner + Expressions.this;
  }

  class Inner {}
}
