/**
 * Short.
 */
class A {
  /** Already one line. */
  int a;
  /**
   * This sentence is long enough that when the comment is collapsed onto one line it is 101 wide..
   */
  int b;
  /**
   * This sentence is long enough that when the comment is collapsed onto one line it is 100 wide.
   */
  int c;
  /**
   * @return the thing
   */
  int d;
  /**
   * Summary.
   * @param x the x
   * @return the thing
   */
  int e;
  /**


   * Leading blank lines.   Multiple   spaces   inside.


   * Second line joined.
   *
   *
   * New paragraph after two blanks.
   * <p>
   * Paragraph tag alone.
   * </p>
   * <p>Para tag attached.</p>
   * <p>
   * Third.</p>
   *
   * @param  x   the x with a very long description that goes on and on and on and on and on and on and wraps
   *            around here
   * @param y
   *            the y
   * @throws IllegalArgumentException if bad
   *
   * @return something
   * @see Foo
   * @since 1.0
   */
  int f;
  /** Summary on first line.
   * More. */
  int g;
  /**
   * Text with {@code inline code that is quite long and should not be broken hopefully} and {@link Foo#bar(int, String) a link label} more words to wrap.
   * A URL https://example.com/a/very/long/url/that/cannot/be/broken/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa end.
   */
  int h;
  /**
   * Text before pre.
   * <pre>
   *   code   here
   *     indented
   * </pre>
   * Text after pre.
   * <pre>{@code
   *   foo();
   * }</pre>
   */
  int i;
  /**
   * List:
   * <ul>
   * <li>First item</li>
   * <li>Second item that is long enough to need wrapping because it has many many many many many many words in it</li>
   *   <li>Third
   *   <ul><li>Nested</li></ul>
   *   </li>
   * </ul>
   * After.
   * <ol>
   *  <li>One
   *  <li>Two
   * </ol>
   */
  int j;
  /**
   * <h2>Heading</h2>
   * Text after heading.
   * <blockquote>Quoted text</blockquote>
   * Line one<br>
   * Line two<br/>Line three
   * <table>
   *   <tr><td>a</td><td>b</td></tr>
   * </table>
   * end
   */
  int k;
  /** */
  int l;
  /**
   */
  int m;
  /***/
  int n;
  /**
   * {@snippet :
   *   foo();
   *     bar();
   * }
   * after
   */
  int o;
}
