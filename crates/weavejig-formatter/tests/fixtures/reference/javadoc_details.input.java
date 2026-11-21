class A {
  /**
   * A.
   * <pre>{@code
   *     deep();
   *   shallow();
   * }</pre>
   * <pre>{@code
   *   foo();
   *   }</pre>
   * <pre>{@code   same line();
   *   foo();
   * }</pre>
   * <pre>
   * {@code
   *   foo();
   * }
   * </pre>
   * <pre>
   *     allIndented();
   *       more();
   * </pre>
   * <pre>
   *line with no space after star
   *   three
   * </pre>
   * {@code   multiple    spaces   {nested {braces}} kept?   } tail
   * {@code multi
   *    line   code} tail
   * <table border="1">
   *  <caption>Cap</caption>
   *   <tr>
   *     <th>h</th>
   *   </tr>
   * </table>
   * text after
   *
   * text after blank after table
   * <pre>x</pre>
   *
   * text after blank after pre
   * <ul><li>x</li></ul>
   *
   * text after blank after list
   * <h2>H</h2>
   *
   * text after blank after heading
   * - dash item one
   * - dash item two
   *
   * 1. numbered
   * 2. numbered
   * </p>
   * <ul>
   * <li>item <ul> <li>inner one <li>inner two </ul> back in outer
   * <li>second <ol><li>deep<ul><li>deeper</li></ul></li></ol>
   * </ul>
   */
  int a;
  /**
   * {@inheritDoc}
   */
  int b;
  /**
   * Returns x. {@inheritDoc}
   *
   * @param <T> type
   * @param x
   * @return
   * @throws Foo
   * @author someone   with  spaces
   * @version 1.0
   * @serial
   * @custom.tag something
   * @
   */
  int c;
  /** First.
   *
   *  Second para. */
  int d;
  /**
   * Summary ends here.
   * @see #foo()
   */
  int e;
  /**
   * @param a the a
   *
   * Text after tag with blank.
   * <pre>
   *   pre in tag
   * </pre>
   * more text
   * @param b the b
   * <ul><li>x</li></ul> trailing
   * @param c the c
   */
  int f;
}
