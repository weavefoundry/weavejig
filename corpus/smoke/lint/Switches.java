package com.example.smoke.lint;

public class Switches {
  int defaults(int n, Object o) {
    switch (n) {
      default:
        return 0;
      case 1:
        return 1;
    }
    switch (n) {
      case 1:
      default:
      case 2:
        return 2;
    }
    switch (n) {
      case 1:
        return 1;
      case 2:
      default:
        return 2;
    }
    switch (n) {
      default -> n++;
      case 1 -> n--;
    }
    int m = switch (n) {
      default -> 0;
      case 1 -> 1;
    };
    switch (n) {
      case 1:
        n++;
    }
    switch (n) {
      case 1 -> n++;
      case 2 -> n--;
    }
    switch (o) {
      case String s -> n++;
      case Integer i -> n--;
      default -> { }
    }
    switch (o) {
      case null -> n++;
      case Object other -> n--;
    }
    return m;
  }

  void falls(int n, boolean flag) {
    switch (n) {
      case 1:
        n++;
      case 2:
        n--;
        // fall through
      case 3:
        n += 2;
        /* falls through */
      case 4: {
        n += 3;
        // fallthru
      }
      case 5:
        if (flag) {
          return;
        } else {
          break;
        }
      case 6:
        if (flag) {
          return;
        }
      case 7:
        while (flag) {
          return;
        }
      case 8:
        try {
          n++;
        } finally {
          return;
        }
      case 9:
        try {
          return;
        } catch (RuntimeException e) {
          n++;
        }
      case 10:
        synchronized (this) {
          throw new IllegalStateException();
        }
      case 11:
        switch (n) {
          case 1:
            return;
          default:
            return;
        }
      case 12:
        for (;;) {
          break;
        }
      case 13:
        n++;
        // not a relief comment
      case 14:
        do {
          continue;
        } while (flag);
      case 15:
        label:
        {
          return;
        }
      case 16:
        n++; // falls through
      case 17:
        n++;
        continue;
      default:
        n = 0;
    }
  }
}
