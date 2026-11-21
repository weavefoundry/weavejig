enum E1 { A, B; void m() {} }
enum E2 { A, B;
  int x;
  int y;
  void m() {}
  int z;
  static {}
  int w;
  { }
  class In {}
  int v;
}
enum E3 {
  A,

  B,
  /** doc */
  C,
  D,
  @Foo E,
  F {
    void m() {}
  },
  G
}
interface I { int A = 1; void m(); void n(); int B = 2;
  int C = 3; }
class K {
  int a;
  static {}
  static {}
  int b;
  int c;
  K() {}
  void m();
  abstract void n();
}
@interface An { int a(); int b() default 1;
  int c(); }
record R(int a) { R {} static int x; }
