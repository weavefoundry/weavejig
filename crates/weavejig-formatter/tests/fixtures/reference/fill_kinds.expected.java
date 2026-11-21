class K {
  void m() {
    a(
        aaaaaaa, a.b, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj, kkkkkkk,
        lllllll, mmmmmmmm);
    b(
        aaaaaaa, f(), ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj, kkkkkkk,
        lllllll, mmmmmmmm);
    c(
        aaaaaaa, a[1], ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj,
        kkkkkkk, lllllll, mmmmmmm);
    d(
        aaaaaaa, a + b, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj,
        kkkkkkk, lllllll, mmmmmm);
    e(
        aaaaaaa, !b, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj, kkkkkkk,
        lllllll, mmmmmmmmm);
    f(
        aaaaaaa, (b), ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj, kkkkkkk,
        lllllll, mmmmmmmm);
    g(
        aaaaaaa, A.B, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj, kkkkkkk,
        lllllll, mmmmmmmm);
    h(
        aaaaaaa, this, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj,
        kkkkkkk, lllllll, mmmmmmm);
    i(
        aaaaaaa, A::b, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj,
        kkkkkkk, lllllll, mmmmmmm);
    j(
        aaaaaaa, x -> 1, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj,
        kkkkkkk, lllllll, mmmmm);
    k(
        aaaaaaa, A.class, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj,
        kkkkkkk, lllllll, mmmm);
    l(
        aaaaaaa, (A) b, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj,
        kkkkkkk, lllllll, mmmmmm);
    n(
        aaaaaaa, new A(), ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj,
        kkkkkkk, lllllll, mmmm);
    o(
        aaaaaaa, bbbbbbbbb, ccccccc, ddddddd, eeeeeee, fffffff, ggggggg, hhhhhhh, iiiiiii, jjjjj,
        kkkkkkk, lllllll, mmm);
    p(
        aaaaaaa,
        bbbbbbbbbb,
        ccccccc,
        ddddddd,
        eeeeeee,
        fffffff,
        ggggggg,
        hhhhhhh,
        iiiiiii,
        jjjjj,
        kkkkkkk,
        lllllll,
        mm);
    q(
        "long string that makes the call too long to fit on one line long long long long long long",
        largeSize,
        smallSize);
    t(
        xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx,
        largeSize,
        smallSize);
    u(
        "long string that makes the call too long to fit on one line long long long long long long",
        1,
        2,
        3,
        4,
        5,
        6);
    v(
        1,
        "long string that makes the call too long to fit on one line long long long long long long",
        3,
        4,
        5,
        6);
    y(
        "short",
        "long string that makes the call too long to fit on one line long long long long long",
        largeSize,
        a,
        b);
    Object[] z = {
      "long string that makes the call too long to fit on one line long long long long long",
      largeSize,
      a,
      b
    };
    Object[] zz = {a, b};
    int[] e1 = {};
    int[] e2 = {
      1,
    };
    int[][] e3 = {{1, 2}, {3, 4}};
    int[][] e4 = {
      {100000000, 200000000, 300000000, 400000000},
      {100000000, 200000000, 300000000, 400000000, 500000000}
    };
    @A({1, 2})
    int q1;
    @A(
        value = {1, 2},
        other = {})
    int q2;
  }
}
