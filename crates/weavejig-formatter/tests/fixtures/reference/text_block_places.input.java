class T {
  String b = """
flush left
  indented
""";
  String b2 = """
flush content
      """;
  String b3 = """
      content
""";

  @Foo("""
      anno
      """)
  void m(boolean c) {
    Supplier<String> s = () -> """
        lambda
        """;
    String t = c ? """
        yes
        """ : """
        no
        """;
    String u = "a" + """
        concat
        """ + "b";
    var v = switch (1) {
      case 1 -> """
          one
          """;
      default -> {
        yield """
            two
            """;
      }
    };
    return """
flush return
""";
  }
  String w = (String) """
      cast
      """;
  String x = """
      one line""".strip();
  String y = """
      esc \\
      q \""" r
      tr\s
      """;
  String z = """ 
      space after opening
      """;
  void n() {
    foo(
        1, """
      a
      """, 2);
    x = """
       y
       """;
    this.field.method("""
       y
       """).other();
    throw new IllegalStateException("""
       boom
       """);
  }
}
