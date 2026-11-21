package com.example.smoke;

import java.io.BufferedReader;
import java.io.FileReader;
import java.io.IOException;

public class Statements {
  private final Object lock = new Object();

  void control(int n, String[] args) throws IOException {
    outer:
    for (int i = 0, j = n; i < j; i++, j--) {
      for (; ; ) {
        if (i == 3) {
          continue outer;
        } else if (i == 4) {
          break outer;
        } else {
          break;
        }
      }
    }

    do {
      n--;
    } while (n > 0);

    switch (n) {
      case 0:
      case 1:
        System.out.println("small");
        break;
      case 2:
        {
          System.out.println("two");
        }
        // fall through
      default:
        System.out.println("other");
    }

    switch (args.length) {
      case 0 -> System.out.println("none");
      case 1 -> {
        System.out.println("one");
      }
      default -> throw new IllegalStateException();
    }

    synchronized (lock) {
      assert n >= 0 : "negative";
      assert args != null;
    }

    try (BufferedReader reader = new BufferedReader(new FileReader(args[0]));
        BufferedReader other = new BufferedReader(new FileReader(args[1]))) {
      System.out.println(reader.readLine() + other.readLine());
    } catch (IOException | RuntimeException e) {
      throw e;
    } finally {
      System.out.println("done");
    }

    try {
      final int local = n;
      var inferred = local + 1;
      int yield = inferred;
      yield = yield + 1;
    } catch (final Exception e) {
      // Ignored.
    }

    if (n > 0) return;
    while (n < 0) n++;
    ;
  }
}
