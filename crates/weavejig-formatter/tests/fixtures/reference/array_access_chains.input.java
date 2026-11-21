class K {
  void m() {
    boolean a1 = !NativeTypeVariableEquals.class.getTypeParameters()[0].equals(newArtificialTypeVariable(NativeTypeVariableEquals.class, "X"));
    boolean a2 = NativeTypeVariableEquals.class.getTypeParameters()[0].equals(newArtificialTypeVariable(NativeTypeVariableEquals.class, "X"));
    boolean a3 = NativeTypeVariableEquals.class.getTypeParameters().equals(newArtificialTypeVariable(NativeTypeVariableEquals.class, "X", 1));
    boolean a4 = nativeTypeVariableEquals.getTypeParameters()[0].equals(newArtificialTypeVariable(NativeTypeVariableEquals.class, "X", 1, 2, 3));
    boolean a5 = nativeTypeVariableEquals.getTypeParameters().equals(newArtificialTypeVariable(NativeTypeVariableEquals.class, "X", 1, 2, 3, 4, 5));
    boolean a6 = nativeTypeVariableEquals[0].equals(newArtificialTypeVariable(NativeTypeVariableEquals.class, "X", 1, 2, 3, 4, 5, 6, 7, 8, 9, 10));
    boolean a7 = getTypeParameters()[0].equals(newArtificialTypeVariable(NativeTypeVariableEquals.class, "X", 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11));
    boolean a8 = getTypeParameters(aaaa).equals(newArtificialTypeVariable(NativeTypeVariableEquals.class, "X", 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11));
    assertThat(someLongExpressionnnnnnnnnnnnnnnnnnnnnnn(aaaaaaaaaaa)).isEqualTo(someOtherLongExpressionnnnnnnnnnnnnn(bbbbbbbbbbbbbbbbbbb));
    assertThat(someLongExpressionnnnnnnnnnnnnnnnnnnnnnn(aaaaaaaaaaa))[0].isEqualTo(someOtherLongExpressionnnnnnnnnnnn(bbbbbbbbbbbbbbbbbbb));
    foo.getTypeParameters()[0].equalssssssssssssssssssss(newArtificialTypeVariable(NativeTypeVariableEquals.class, "X", 1, 2, 3)).bar();
  }
}
