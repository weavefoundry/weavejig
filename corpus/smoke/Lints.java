package com.example.smoke;

import java.util.*;

public class Lints {
  final static long TOO_SMALL = 10l;
  static public int counter;
  int numbers[];

  void run(String args[], int n) {
    if (n > 0) counter++;
    else counter--;
    for (int i = 0; i < n; i++) counter += i;
    while (n-- > 0);
    do counter++; while (counter < 10);
    ;
    long ok = 20L;
  }
}
