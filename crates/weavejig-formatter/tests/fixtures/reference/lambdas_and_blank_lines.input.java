class K {
  void m() {
    new Thread(() -> { a(); }).start();
    foo(() -> { a(); }).start();
    Foo.of(() -> { a(); }).start();
    new Thread(() -> { a(); });
    x = new Thread(() -> { a(); });
    new Thread(() -> aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, ccccccccccccccccccc, dddddddd));
    new Thread(() -> aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, ccccccccccccccccccc));
    foo.bar(() -> aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, ccccccccccccccccccc));
    foo.bar(() -> aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, ccccccccccccccccccc, ddd));
    if (a) {
      b();

    } else if (c) {

      d();

    } else {
      e();

    }
    try {
      a();

    } catch (E e) {
      b();

    } finally {
      c();

    }
    do {
      a();

    } while (x);
    foo(new Object() {
      int x;

    });
    Runnable r = () -> {
      a();

    };
    switch (x) {
      case 1:
        a();

      case 2:
        b();

    }
  }

}
enum E1 {

  /** doc */
  A,
  B
}
enum E2 {

  // comment
  A,
  B
}
/** doc */

@Foo
class L {}
/** doc */

class M {
  /** doc */

  int x;
  // c

  int y;
}
