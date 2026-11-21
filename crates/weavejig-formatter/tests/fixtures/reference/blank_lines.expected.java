package p;

import foo.Bar;
import foo.Baz;

class T {
  @Bar @Baz final Object f7 = null;
  @Bar final Object f8 = null;
  @Bar @Baz Object f9;

  @Bar
  @Baz("x")
  Object f10;

  @Bar
  public @Baz("x") Object f11;

  int plain1;

  /** doc */
  int doc1;

  int plain2;
  // line comment
  int commented;
  int plain3;
  int multi =
      aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
          + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
          + cccccccccccccccccccccccccccccccc;
  int plain4;
  @Bar int anno1;
  int plain5;
  /* block */
  int blockc;
  int plain6;

  void m() {

    @Bar
    @Baz
    Object l8 = null;
    @Bar final Object l9 = null;
    @Bar
    @Baz
    final Object l7 = null;
    final @Bar @Baz Object l10 = null;
  }

  void n() {
    // comment first

    int x;
  }

  void o() {

    // comment after blank
    int x;
  }

  void p() {

    if (x) {

      y();
    }
    switch (x) {
      case 1:
        foo();
    }
    Runnable r =
        () -> {
          foo();
        };
    new Object() {

      int x;
    };
  }
}

class U {

  int x;
}

enum E {
  A,
  B;

  int x;
}
