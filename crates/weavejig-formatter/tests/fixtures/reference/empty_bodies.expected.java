class D {
  void m(int n) {
    while (n-- > 0)
      ;
    do n++;
    while (n < 3);
    do {
      n++;
    } while (n < 3);
    if (n > 0)
      ;
    if (n > 0)
      ;
    else
      ;
    for (; ; )
      ;
    for (int x : xs)
      ;
    do
      ;
    while (n < 3);
    do n++;
    while (n < 3
        && someOtherVeryLongConditionThatGoesOnAndOn(withArguments, andMoreArguments)
        && more());
  }
}
