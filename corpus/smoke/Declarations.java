package com.example.smoke;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.util.List;

public abstract class Declarations implements Comparable<Declarations>, Cloneable {
  static {
    System.out.println("static init");
  }

  {
    System.out.println("instance init");
  }

  @Retention(RetentionPolicy.RUNTIME)
  @Target({ElementType.METHOD, ElementType.TYPE})
  @interface Marker {
    String value() default "";

    int[] numbers() default {1, 2, 3};

    Class<?> type() default Object.class;
  }

  enum Planet implements Comparable<Planet> {
    MERCURY(3.303e+23, 2.4397e6),
    @Deprecated
    VENUS(4.869e+24, 6.0518e6),
    EARTH(5.976e+24, 6.37814e6) {
      @Override
      double surfaceGravity() {
        return 9.8;
      }
    };

    private final double mass;
    private final double radius;

    Planet(double mass, double radius) {
      this.mass = mass;
      this.radius = radius;
    }

    double surfaceGravity() {
      return 6.67300E-11 * mass / (radius * radius);
    }
  }

  enum Empty {}

  enum Simple {
    A,
    B,
    C,
  }

  record Point(int x, int y) implements java.io.Serializable {
    static final Point ORIGIN = new Point(0, 0);

    Point {
      if (x < 0 || y < 0) {
        throw new IllegalArgumentException();
      }
    }

    Point(int both) {
      this(both, both);
    }

    double norm() {
      return Math.sqrt(x * x + y * y);
    }
  }

  record Pair<A, B>(A first, @Marker("b") B second) {}

  sealed interface Shape permits Circle, Square {}

  static final class Circle implements Shape {}

  static non-sealed class Square implements Shape {}

  interface Visitor<R> {
    R visit(Shape shape);

    default R visitAll(List<? extends Shape> shapes) throws Exception {
      R last = null;
      for (Shape shape : shapes) {
        last = visit(shape);
      }
      return last;
    }

    static <R> Visitor<R> constant(R value) {
      return shape -> value;
    }
  }

  protected transient volatile int counter;
  private int a = 1, b, c[] = {1};

  protected Declarations() {
    super();
  }

  public abstract void run(String... args) throws InterruptedException, java.io.IOException;

  @Marker(value = "x", numbers = {4, 5})
  public synchronized native int nativeMethod(final int first, int[] rest);

  @Override
  public int compareTo(Declarations other) {
    class Local {
      int value() {
        return 1;
      }
    }
    return new Local().value();
  }
}
