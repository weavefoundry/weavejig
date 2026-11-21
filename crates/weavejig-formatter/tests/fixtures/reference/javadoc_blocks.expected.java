class A {
  /** xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxx. */
  int len89;

  /** xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx. */
  int len90;

  /** xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx . */
  int len91;

  /**
   * xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx x.
   */
  int len92;

  /**
   * Pre variants.
   *
   * <pre>{@code
   * foo();
   *   bar();
   *
   * baz();
   * }</pre>
   *
   * <pre>
   * no indent
   *    three
   * </pre>
   *
   * <pre>one line</pre>
   *
   * <pre><code>
   *   in code
   * </code></pre>
   *
   * text
   *
   * <pre>inline start
   *   more</pre>
   *
   * trailing text
   *
   * <pre class="x">
   *  attr
   * </pre>
   */
  int p;

  /**
   * Item list:
   *
   * <ul>
   *   <li>
   *       <p>Para in item.
   *       <p>Second para in item.
   *   <li>{@code code} item with trailing text. continuation
   *   <li>Item with pre:
   *       <pre>
   *     x
   *   </pre>
   * </ul>
   *
   * <dl>
   *   <dt>Term
   *   <dd>Definition text
   * </dl>
   *
   * <p>
   *
   * <ul>
   *   <li>after p
   * </ul>
   *
   * <h3>Head</h3>
   *
   * <p>After head. Text <b>bold</b> and <i>italic</i> and <a
   * href="https://example.com/some/long/link/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">link
   * text here</a>.
   * <!-- a comment -->
   * Trailing. <hr> After hr.
   *
   * @param a first
   *     <p>para inside tag
   *     <ul>
   *       <li>list in tag
   *     </ul>
   *
   * @param b second
   *     <p>continued after blank
   * @deprecated use {@link #other()} because this text is long enough to require wrapping, so we
   *     see indent
   */
  int q;

  /**
   * Summary.
   *
   * <p>Loose p.
   *
   * <p>Upper p.
   */
  int r;

  /**
   * Not a block tag: foo@bar.com and {@literal @}Override and
   *
   * @notATagMidSentence? This line starts with at-sign.
   *     <p>Paragraph two starting with <code>code</code>.
   *     <p><b>Bold start paragraph.</b>
   *     <p>{@code Code start paragraph.}
   *     <p><a href="x">Link start</a> paragraph.
   *     <ul>
   *       <li>x
   *     </ul>
   *     <pre>x</pre>
   *     <h2>H</h2>
   *
   * @see A
   */
  int s;

  /* plain block comment
  stays as is */
  int t;

  /** Misaligned stars here not starred line */
  int u;

  void m() {
    /**
     * Javadoc inside method body that is long enough to need wrapping when it is formatted by the
     * tool okay.
     */
    int x;
  }
}
