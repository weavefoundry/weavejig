package com.example.smoke.lint;

public class Booleans {
  boolean simplify(boolean a, boolean b, String s, Object o) {
    if (a == true) {
      return false;
    }
    if (!false || b != false) {
      return true;
    }
    boolean c = a && true;
    boolean d = (true) || b;
    boolean e = a ? true : false;
    boolean f = a ? b : false;
    boolean g = a ? true : b;
    boolean h = !(true);
    boolean i = a == (b == true);
    boolean j = a ? false : true;
    boolean k = true ? a : b;
    if (a) {
      return true;
    } else {
      return false;
    }
  }

  boolean returns(boolean a, int n) {
    if (n > 1) return true; else return false;
    if (n > 2) {
      return false;
    } else {
      return true;
    }
    if (n > 3) {
      return true;
    } else {
      return a;
    }
    if (n > 4) {
      n++;
      return true;
    } else {
      return false;
    }
    if (n > 5) {
      return true;
    }
    return false;
  }

  boolean strings(String s, String t, Object o) {
    boolean a = s == "a";
    boolean b = "b" != t;
    boolean c = s == t;
    boolean d = (s) == ("d");
    boolean e = s == "e" + t;
    boolean f = s == t + "f";
    boolean g = s == """
        text""";
    boolean h = o == null;
    boolean i = s.equals("i");
    boolean j = s == ("j" + "k");
    boolean k = 'c' == s.charAt(0);
    return a;
  }
}
