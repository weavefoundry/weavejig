package smoke;

/** Type annotations on array brackets and on a varargs ellipsis. */
class AnnotatedArrays {
  static final ThreadLocal<int @Nullable []> hashCodes = new ThreadLocal<>();
  private transient int @Nullable [] entries;

  @Nullable
  Object @Nullable [] @Size(2) [] grid(@Nullable Object @Nullable [] values, String @NonNull ... rest) {
    int @Nullable [] local = new int @Nullable [3] @NonNull [];
    Object first = (@Nullable Object @Nullable []) values;
    int legacy @Nullable [] = {};
    for (int @Nullable [] row : new int[0][]) {}
    return new Object @Nullable [] @Size(2) [] {null};
  }
}
