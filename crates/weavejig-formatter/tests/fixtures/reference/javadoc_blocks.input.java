class A {
  /**
   * xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxx.
   */
  int len89;
  /**
   * xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx.
   */
  int len90;
  /**
   * xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx .
   */
  int len91;
  /**
   * xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx xxxxxxxxx x.
   */
  int len92;
  /**
   * Pre variants.
   * <pre>{@code
   *   foo();
   *     bar();
   *
   *   baz();
   * }</pre>
   * <pre>
   * no indent
   *    three
   * </pre>
   * <pre>one line</pre>
   * <pre><code>
   *   in code
   * </code></pre>
   * text <pre>inline start
   *   more</pre> trailing text
   * <pre class="x">
   *  attr
   * </pre>
   */
  int p;
  /**
   * Item list:
   * <ul>
   *   <li><p>Para in item.</p>
   *   <p>Second para in item.</p></li>
   *   <li>{@code code} item with trailing text.
   *       continuation</li>
   *   <li>Item with pre:
   *   <pre>
   *     x
   *   </pre>
   *   </li>
   * </ul>
   * <dl>
   *   <dt>Term</dt>
   *   <dd>Definition text</dd>
   * </dl>
   * <p>
   * <ul><li>after p</li></ul>
   * <h3>Head</h3>
   * <p>After head.</p>
   * Text <b>bold</b> and <i>italic</i> and <a href="https://example.com/some/long/link/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">link text here</a>.
   * <!-- a comment -->
   * Trailing.
   * <hr>
   * After hr.
   *
   * @param a first
   * <p>para inside tag</p>
   * <ul><li>list in tag</li></ul>
   * @param b second
   *
   *          continued after blank
   * @deprecated use {@link #other()} because this text is long enough to require wrapping, so we see indent
   */
  int q;
  /**
   * Summary.
   *
   * <p>
   *
   * Loose p.
   *
   * <P>Upper p.
   */
  int r;
  /**
   * Not a block tag: foo@bar.com and {@literal @}Override and
   * @notATagMidSentence? This line starts with at-sign.
   *
   * Paragraph two starting with <code>code</code>.
   *
   * <b>Bold start paragraph.</b>
   *
   * {@code Code start paragraph.}
   *
   * <a href="x">Link start</a> paragraph.
   *
   * <ul><li>x</li></ul>
   *
   * <pre>x</pre>
   *
   * <h2>H</h2>
   *
   * @see A
   */
  int s;
  /* plain block comment
       stays as is */
  int t;
  /**
       * Misaligned stars
     *   here
  not starred line
   */
  int u;
  void m() {
    /** Javadoc inside method body that is long enough to need wrapping when it is formatted by the tool okay. */
    int x;
  }
}
