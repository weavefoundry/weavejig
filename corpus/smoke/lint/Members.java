package com.example.smoke.lint;

import java.util.function.IntBinaryOperator;

public class Members {
  int zero = 0;
  long zeroLong = 0L;
  double zeroDouble = 0.0;
  float zeroFloat = 0f;
  char zeroChar = '\0';
  char otherChar = 'a';
  boolean no = false;
  boolean yes = true;
  Object nothing = null;
  String text = null, other = "x";
  int[] array = null;
  int brackets[] = null;
  Integer boxed = null;
  int one = 1;
  int hex = 0x0;
  int mask = 0x0f;
  final int constant = 0;
  static int shared = 0;
  int negative = -0;
  int cast = (int) 0;
  byte small = 0;
  short tiny = 0;
  char number = 0;

  int a, b;
  int c; int d;
  long big = 10l, bigger = 20L;
  int values[] = {1}, more[];

  interface Constants {
    int ZERO = 0;
    Object NOTHING = null;
  }

  void locals(int[] params, int old[]) {
    int local = 0;
    int e, f;
    int g; int h;
    for (int i = 0, j = 1; i < j; i++) {}
    int k[] = null;
    java.util.List<String>[] lists = null;
    IntBinaryOperator sum = (int x, int y) -> x + y;
    int trailing[][] = {};
    ; ;
  }

  int legacy()[] {
    return null;
  }

  void statements(int n) {
    n++; n--;
    n = 1
    ; n = 2;
    for (;;) { n++; }
    if (n > 0) { n++; } n--;
    Runnable r = () -> { n++; }; r.run();
    int v = 1; v++;
    do n++; while (n < 3); n = 4;
    {
      n++;
    }
    {}
    switch (n) {
      case 1: {
        break;
      }
      case 2: {
        n++;
      }
      break;
      default:
    }
    label: {
      n++;
    }
    synchronized (this) {
      {
        n++;
      }
    }
  }
}
