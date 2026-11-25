package com.example.smoke.lint;

import java.util.function.BiFunction;

public class Names<type, T> {
  static final int lowerConstant = 1;
  static final int UPPER_CONSTANT = 2;
  static final long serialVersionUID = 1L;
  static int Static_Field;
  int Member_Field;
  final int FINAL_MEMBER = 3;
  private int _underscore;

  void Method_Name(int Param_Name, final int OTHER) {
    int Local_Name = 1;
    final int FINAL_LOCAL = 2;
    for (int I = 0; I < 1; I++) {}
    for (String Each : new String[0]) {}
    try (java.io.StringReader Reader_Name = new java.io.StringReader("")) {
    } catch (RuntimeException Problem) {
    } catch (final Error E) {
    } catch (IllegalStateError | java.io.IOException ee) {
    }
    BiFunction<Integer, Integer, Integer> f = (First_Arg, second) -> First_Arg;
    Runnable r = () -> {};
    java.util.function.Function<String, String> g = Bad_Name -> Bad_Name;
    Object o = Local_Name;
    if (o instanceof String Pattern_Name) {}
    if (o instanceof Point(int X_COORD, var y)) {}
  }

  void Names() {}

  <bad, U> void generic() {}

  record Point(int X, int y) {}

  record Pair<first, S>(first a, S b) {}

  interface lower_interface<elem> {
    int interface_field = 1;
  }

  enum lower_enum {
    value
  }

  @interface lower_annotation {}

  class XMLHTTPRequest {
    int parseXMLDocument;
    int userID;
    int HTMLParserFOO;
    static int STATICVALUE;
    final int FINALVALUE = 1;
    static final int CONSTANTVALUE = 2;

    void doXMLHTTP() {}

    @Override
    public String toSTRINGX() {
      return "";
    }

    void params(int theURLXY, int abCDEFg) {}
  }
}
