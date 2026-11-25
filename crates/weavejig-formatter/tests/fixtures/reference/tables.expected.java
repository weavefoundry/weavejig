class L {
  int[] p1 = {1, 2, 3, 4, "s", 6};
  int[] p2 = {1, 2, 3, 4, 5, 6, "s", 8};
  int[] p2b = {1, 2, "t", 4, "s", 8};
  int[] p2c = {1, 2, 3, "x", 5, x, 7, y()};
  int[] p2d = {1, 2, 3, "x", 5, x, 7, 8};
  int[] p3 = {
    1, 2, 3,
    4, 5, 6,
    7
  };
  int[] p4 = {1, 2, 3, 4, 5};
  int[] p5 = {1, 2, 4, 5, 6, 7, 8, 9};
  Object[] p6 = {1, "a", foo()};
  Object[] p7 = {1, 2, 3};
  Object[] p7b = {1, 2};
  Object[] p7c = {1, 2};
  Object[] p8 = {1, "a", "b", 2};
  Object[] p9 = {1, 2, 3, 4, 5, 6};
  Object[] p10 = {
    0, -1,
    -8, 2,
    127, 127
  };
  Object[] p11 = {
    aaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbb,
    cccccccccccccccccccc, ddddddddddddddddddddddddd,
  };
  Object[] p12 = {
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
        27, 28, 29, 30, 31, 32, 33,
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
        27, 28, 29, 30, 31, 32, 33
  };
  Object[] p13 = {
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  };

  void m() {
    f(
        1, 2,
        3, 4);
    f(1, 2, 3, 4, 5, 6);
    f(1, 2, 3, 4, 5, 6, 7, 8, 9);
    f(a, 2, "s", 4, b, 6);
    f(a, 2, b, "s", c, x);
    f(a, 2, b, 3, c, 4);
    f(a, b, c);
    f(
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
            bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
            bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
  }
}
