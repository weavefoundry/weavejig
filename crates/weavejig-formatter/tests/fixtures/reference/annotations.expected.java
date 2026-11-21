package p;

import foo.Bar;
import foo.Baz.Qux;
import javax.annotation.CheckForNull;
import org.checkerframework.checker.nullness.qual.NonNull;
import org.jspecify.annotations.Nullable;

class T {
  @Nullable Object a() {
    return null;
  }

  @CheckForNull
  Object b() {
    return null;
  }

  @NonNull Object c() {
    return null;
  }

  @Bar
  Object d() {
    return null;
  }

  @Override
  @Nullable
  public Object e() {
    return null;
  }

  @Override
  public @Nullable Object f() {
    return null;
  }

  @Nullable
  @Override
  Object g() {
    return null;
  }

  @Bar
  @Nullable Object h() {
    return null;
  }

  @Nullable
  @Bar
  Object i() {
    return null;
  }

  @Nullable <T> T j() {
    return null;
  }

  @Nullable
  T() {}

  @Nullable Object field;
  @Bar Object field2;

  @Bar("x")
  Object field3;

  @Bar("x")
  @Nullable Object field4;

  @Nullable
  @Bar("x")
  Object field5;

  @Bar @Baz private Object field6;

  @Nullable
  class Inner {}

  void m(@Bar("x") int p, @Nullable Object q) {
    @Bar Object l1 = null;
    @Bar("x")
    Object l2 = null;
    @Nullable Object l3 = null;
    @Bar("x")
    @Nullable Object l4 = null;
    @Bar("x")
    final Object l5 = null;
    final @Bar("x") Object l6 = null;
    @Bar
    @Baz
    final Object l7 = null;
    for (@Bar("x") Object o : list) {}
    try (@Bar("x")
        Res r = open()) {}
  }

  @org.jspecify.annotations.Nullable
  Object k() {
    return null;
  }

  @Qux
  Object n() {
    return null;
  }
}
