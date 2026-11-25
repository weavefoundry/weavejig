package com.example.smoke.lint;

import java.util.Objects;

interface Sibling {
  int LIMIT = 3;

  class Holder {
    private Holder() {}
  }
}

class Second {
  private Second() {}

  static int helper() {
    return Objects.hash(1);
  }
}

class Utility {
  public static final int LIMIT = 1;

  static int twice(int n) {
    return n + n;
  }
}

final class Third extends Utility {
  final void locked() {}

  private final void hidden() {}
}

enum Fourth {
  ONE;

  private Fourth() {}

  ;
}
