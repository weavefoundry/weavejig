class LongStrings {
  @SuppressWarnings("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore")
  static final String FIELD = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore";

  String[] array = {"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore", "b"};

  void words() {
    foo("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.");
    String s = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";
    foo(1, 2, "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.", x);
    a.b().c("ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab").d();
  }

  void existingConcatenations() {
    // Pieces that fit are left alone.
    foo("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod " + "tempor incididunt ut labore et dolore magna aliqua.");
    // Otherwise the literals are merged and split again.
    foo("short one " + "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.");
    foo("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et" + " short");
    foo("Lorem ipsum dolor sit amet, "
        + "consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua");
    // Only runs of literals are merged.
    foo("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod " + x + "tempor incididunt ut labore et dolore magna aliqua, sed do eiusmod tempor incididunt.");
    foo(x + "aaa " + "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore");
    foo("ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab" + x);
  }

  void breaks() {
    // After `\n`, and before any space.
    foo("line one\nline two\nline three Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod");
    foo("Lorem\nipsum dolor sit amet,\tconsectetur adipiscing elit, sed do eiusmod tempor\\ incididunt ut labore\" et dolore magna aliqua.");
    foo("ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab\n\nab\n");
    foo("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididu     ut labore");
    foo("    Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.   ");
    // A word too long for any line.
    foo("Lorem ipsum dolor aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa sit amet, consectetur adipiscing elit");
    foo("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    // A piece shorter than five characters is not broken off.
    foo("abcd aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    foo("abcde aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    foo("ab " + "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    foo("abcdefgh " + "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
  }

  void boundaries() {
    // A word too long for any line is cut where a literal started, if the part before fits.
    foo("ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab" + "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    // Otherwise where literals started makes no difference.
    foo("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut " + "labore et dolore magna aliqua.");
    foo("ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab " + "https://example.com/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/" + "bbbbbbbbbb cc");
    // Text with nowhere to break stays as it was split.
    String base64 =
        "rO0ABXNyAB5vcmcuYXBhY2hlLmNvbW1vbnMubGFuZzMuUmFuZ2UAAAAAAAAAAQIAA0wACmNvbXBhcmF0b3J0ABZMamF2YS91dGls"
            + "L0NvbXBhcmF0b3I7TAAHbWF4aW11bXQAEkxqYXZhL2xhbmcvT2JqZWN0O0wAB21pbmltdW1xAH4AAnhwfnIAM29yZy5hcGFjaGUu";
    // A comment between literals ends a run.
    foo(("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore " + //
        "et dolore magna aliqua.").formatted(x));
  }

  void trailing() {
    // The last piece leaves room for what follows it.
    foo("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incidid ut");
    foo("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incidi ut");
    if (s.equals("ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab abc")) {
      x();
    }
    foo("ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab"); // c
    foo("ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab ab");
  }

  Object indentation() {
    // New lines are indented from the literal, not laid out again.
    String t = c ? "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore" : "b";
    Supplier<String> l = () -> "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore";
    switch (s) {
      case "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore":
        break;
    }
    return "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore";
  }

  void unchanged() {
    // What would change meaning, and text blocks.
    int n = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore".length();
    String t = """
        Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore
        """;
  }
}
