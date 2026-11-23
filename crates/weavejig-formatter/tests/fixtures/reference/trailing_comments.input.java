class TrailingComments {
  // A trailing comment breaks the line after it, and only what encloses that
  // line break wraps: the chain the comment trails stays on one line.
  boolean isSeedless(Method method) {
    return method.getReturnType().equals(HashFunction.class) // must return HashFunction
        && Modifier.isPublic(method.getModifiers()) // only the public methods
        && method.getParameterTypes().length == 0 // only the seedless hash functions
        && !legacyHashingMethodNames.contains(method.getName());
  }

  void statements() {
    if (in == null) inReadFully = true; // effectively an empty stream
    if (in != null) // for null-wrapped streams
      inReadFully = false;
    while (more) // why
      next();
    for (int i = 0; i < n; i++) // each
      visit(i);
    if (ready) go(); // now
    else stay();
    return // nothing
        ;
  }

  void calls() {
    foo(a, b // last
    );
    foo(aaaa, // first
        bbbb, cccc);
    foo(a.b(), c.d(), // second
        e.f());
    foo(bar(aaaa, // inner
        bbbb), cccc);
    foo( // before
        a, b);
    assertThat(x).isEqualTo(y.build() //
    );
    var events = executeTests(request().selectors(selectClass(Foo.class)).configurationParameter("a", "b").build() //
    );
    String s = foo.bar() // c
        .baz().qux();
  }

  void operators() {
    int a = aaaa + // c
        bbbb + cccc;
    int b = aaaa // c
        + bbbb + cccc;
    int c = aaaa * bbbb + // c
        cccc * dddd;
    boolean d = a.b() && // c
        c && d;
    String e = "aaaa" + // c
        "bbbb" + "cccc";
    x = foo(a) // c
        + bar(b);
    x = cond ? a // c
        : b;
    x = cond // c
        ? a : b;
    int f = aaaaaa + bbbbbb + cccccc + dddddd + eeeeee + ffffff + gggggg + hhhhhh + iiiiii + jjjjjj + kkkkkk;
    int g = aaaaaaaaaaa + bbbbbb + cccccc + dddddd + eeeeee + ffffff + gggggg + hhhhhh + iiiiii + jjjjjj;
  }

  int[] values = {1, 2, // c
      3, 4};
}
