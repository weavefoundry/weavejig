class T {
  String a = """
      hello
        world
      """;
  String b =
"""
flush left
  indented
""";
	String c = """
		tabbed
			more
		""";
  String d = """
      closing on content line""";
  String e = """
          deep
      shallow closing
   """;
  String f = """
      line one \
      line two\
      """;
  String g = """
      a

      b   
      """;
  String h = """
      """;

  void m() {
    call("x", """
        arg content
        """);
    call(
        """
    less indented
    """,
        2);
    String s = """
      text
      """.formatted(1);
    assertEquals("""
            deep in source
              keep relative
            """, actual);
    foo("""
  x""" + y);
    return """
        ret
        """;
  }

  static final String DEEP =
      String.join(
          ",",
          """
          one
          """,
          """
                    two""");
}
