package com.example.smoke.lint;

/**
 * A class.
 *
 * @deprecated use something else
 */
public class Documented {
  /** {@inheritDoc} */
  public String toString() {
    return "";
  }

  /** {@inheritDoc} */
  @Override
  public int hashCode() {
    return 1;
  }

  /** {@inheritDoc} */
  @java.lang.Override
  public boolean equals(Object o) {
    return false;
  }

  /** {@inheritDoc} */
  static void helper() {}

  /** {@inheritDoc} */
  private void hidden() {}

  /** Mentions inheritDoc but has no tag. */
  void plain() {}

  /**
   * Multi-line.
   *
   * <p>{@inheritDoc}
   */
  void later() {}

  // {@inheritDoc}
  void lineComment() {}

  /** @deprecated use {@link #plain()} */
  void tagOnly() {}

  @Deprecated
  void annotationOnly() {}

  /** Has documentation but no tag. */
  @Deprecated
  void annotationAndDoc() {}

  /** @deprecated both */
  @Deprecated
  void both() {}

  /** @deprecated field */
  int field;

  /** A field. */
  @Deprecated int other;

  /** @deprecated constructor */
  Documented() {}

  /** Uses the word @deprecated mid-sentence. */
  void midSentence() {}

  /** @deprecated nested */
  interface Nested {
    /** @deprecated method */
    void run();
  }

  /** An enum. */
  enum Level {
    /** @deprecated constant */
    LOW,
    /** Fine. */
    @Deprecated
    HIGH
  }

  /** @deprecated annotation */
  @interface Marker {
    /** Element. */
    @Deprecated
    int value() default 0;
  }

  void locals() {
    /** @deprecated local */
    int local = 1;
  }
}
