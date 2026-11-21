package x;

// header comment

import a.A;
import b.B; // why b

/** Doc. */
@Deprecated
@SuppressWarnings({"unchecked", "rawtypes"})
public class Long<T> extends Base<T> implements Runnable, Comparable<Long<T>> {
  @Inject
  @Named("x")
  private Foo foo;

  @Nullable private Bar bar;

  /**
   * Misindented javadoc.
   * @param a first
   */
  @Override
  public <R> R method(@Nullable final String a, int b) throws IOException, InterruptedException {
    String result =
        someObject.someMethodWithLongName(firstArgumentIsLong, secondArgumentIsAlsoLong, third);
    boolean ok =
        firstConditionIsRatherLong && secondConditionIsAlsoQuiteLong
            || thirdConditionGoesOverTheLimit && d;
    int value =
        condition
            ? someValueThatIsVeryLongIndeedAndGoesOnAndOn
            : otherwiseThisOtherValueWhichIsAlsoLong;
    list.stream()
        .filter(x -> x.isActive() && x.getValue() > 10)
        .map(x -> x.getName())
        .collect(Collectors.toList());
    callback(
        arg,
        x -> {
          doIt(x);
        });
    int[] big = {
      1000000,
      2000000,
      3000000,
      4000000,
      5000000,
      6000000,
      7000000,
      8000000,
      9000000,
      10000000,
      11000000
    };
    if (a == null) /* inline */ return null;
    else b++;
    x = -(-y) + - -z + (int) (a + b) - (T) c + a >>> 2 >> 1;
    for (; ; ) {
      break;
    }
    switch (b) {
      case 1:
      case 2:
        foo();
        break;
      default:
        {
          bar();
        }
    }
    try {
      run();
    } catch (final Exception e) {
      /* ignored */
    } finally {
    }
    Object o = new int[2][3], p = this.<String>id("x"), q = Foo.<A, B>make(), r = String[]::new;
    label:
    while (true) {
      continue label;
    }
    return (R) result; // done
  }

  enum E {
    A,
    B(1) {
      void f() {}
    },
    @Deprecated
    C;
    int x;
  }

  interface I {
    void f();

    int g();
  }

  record R(int a, String b) {}

  @interface Ann {
    String value() default "x";
  }
  // dangling
}
// end
