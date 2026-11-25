package com.example.smoke;

import com.example.smoke.*;
import java.lang.*;
import java.lang.annotation.*;
import java.util.*;
import com.example.smoke.Sibling;
import java.lang.String;
import com.example.smoke.sub.*;
;
@A()
class R {
  ;
  @B() int x;;
  int z; // trailing
  ; // comment on empty
  enum E { ONE, TWO;; int q; }
  enum F { ONE; }
  enum G { ONE,; }
  void m(@D() int p) {
    ;
    int a = 1;;
    ; // keep?
    /* lead */ ;
    f((u) -> u + 1, (v, w) -> v, (int t) -> t, () -> 1, (s) -> { return s; });
    switch (a) {
      case 1:
        ;
        break;
      default:
        ;
    }
    for (;;) { ; }
    label: ;
    if (a > 0) { ; } else ;
  }
  ;
}
;
