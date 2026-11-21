class K {
  void m() {
    while (x) {}
    for (;;) {}
    for (A a : b) {}
    if (x) {}
    if (x) {} else {}
    if (x) { a(); } else {}
    if (x) {} else if (y) {} else { b(); }
    try {} catch (E e) {}
    try { a(); } catch (E e) {} finally {}
    try { a(); } finally {}
    try (R r = o()) {}
    try (R r = o()) {} catch (E e) {}
    synchronized (x) {}
    {}
    do {} while (x);
    switch (x) {}
    switch (x) { case 1: {} case 2 -> {} default -> {} }
    Runnable r = () -> {};
    label: {}
    new Object() {};
    try { a(); } catch (E e) { /* ignored */ }
    try { a(); } catch (E e) {
      // ignored
    }
    if (x) { /* c */ }
  }
}
