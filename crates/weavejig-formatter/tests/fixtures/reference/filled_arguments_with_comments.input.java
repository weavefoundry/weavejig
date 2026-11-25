class F {
  void m() {
    // A long comment after the last argument: one argument per line.
    f(
        i1, i2, i3, // Integer subscribers are dispatched to first.
        s1, s2, // Though each integer subscriber dispatches to all string subscribers,
        s1, s2, // those string subscribers aren't actually dispatched to until all integer
        s1, s2 // subscribers have finished.
        );
    // Short comments: the arguments fill lines.
    f(
        i1, i2, i3, // a
        s1, s2, // b
        s1, s2, // c
        s1, s2 // d
        );
    // No comment after the last argument: the arguments fill lines.
    f(
        i1, i2, i3, // Integer subscribers are dispatched to first.
        s1, s2, // Though each integer subscriber dispatches to all string subscribers,
        s1, s2, // those string subscribers aren't actually dispatched to until all integer
        s1, s2
        );
    // Two rows, long comment last.
    f(
        i1, i2, i3, // Integer subscribers are dispatched to first.
        s1, s2 // subscribers have finished.
        );
    // Three rows, short comments.
    f(
        i1, i2, i3, // a
        s1, s2, // b
        s1, s2 // c
        );
    g(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bb); //
    g(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bb); //
    g(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bb); // x
    g(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bb); // x
  }
}
