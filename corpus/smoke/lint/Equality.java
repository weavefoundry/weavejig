package com.example.smoke.lint;

public class Equality {
  static class OnlyEquals {
    @Override
    public boolean equals(Object o) {
      return o == this;
    }
  }

  static class OnlyHashCode {
    @Override
    public int hashCode() {
      return 1;
    }
  }

  static class Both {
    @Override
    public boolean equals(Object o) {
      return o == this;
    }

    @Override
    public int hashCode() {
      return 1;
    }
  }

  static class Covariant {
    public boolean equals(Covariant other) {
      return other == this;
    }
  }

  static class CovariantWithObject {
    public boolean equals(CovariantWithObject other) {
      return other == this;
    }

    @Override
    public boolean equals(Object other) {
      return other == this;
    }

    @Override
    public int hashCode() {
      return 1;
    }
  }

  static class Qualified {
    public boolean equals(java.lang.Object other) {
      return other == this;
    }
  }

  static class NotPublic {
    boolean equals(Object other) {
      return other == this;
    }

    protected int hashCode(int seed) {
      return seed;
    }
  }

  static class TwoArguments {
    public boolean equals(Object a, Object b) {
      return a == b;
    }

    public static boolean equals(String a) {
      return a == null;
    }
  }

  interface Shape {
    boolean equals(Shape other);
  }

  enum Kind {
    ONE {
      public boolean equals(Kind other) {
        return false;
      }
    };
  }

  record Point(int x) {
    public boolean equals(Point other) {
      return other.x == x;
    }
  }

  Object anonymous = new Object() {
    public boolean equals(String other) {
      return false;
    }

    @Override
    public int hashCode() {
      return 2;
    }
  };

  abstract static class AbstractEquals {
    @Override
    public abstract boolean equals(Object o);
  }

  static class NativeHash {
    @Override
    public native int hashCode();
  }
}
