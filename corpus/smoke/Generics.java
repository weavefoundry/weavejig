package com.example.smoke;

import java.io.Serializable;
import java.util.Collection;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.function.BiFunction;
import java.util.function.Function;
import java.util.function.Supplier;

public class Generics<T extends Comparable<? super T> & Serializable, U> {
  private Map<String, List<Map<T, ? extends U>>> nested;
  private List<List<List<String>>> deep;

  public <R> List<R> map(Collection<? extends T> items, Function<? super T, ? extends R> f) {
    List<R> out = new java.util.ArrayList<R>();
    for (T item : items) {
      out.add(f.apply(item));
    }
    return out;
  }

  public static <K, V extends Comparable<V>> K maxKey(Map<K, V> map) {
    return map.entrySet().stream()
        .max(Map.Entry.<K, V>comparingByValue())
        .map(Map.Entry::getKey)
        .orElse(null);
  }

  @SuppressWarnings("unchecked")
  T cast(Object o) {
    Object copy = (Object) o;
    int n = (int) 3.5;
    long shifted = (long) n >> 2 >>> 1 << 3;
    boolean less = n < 4 && n > 2;
    Runnable r = (Runnable & Serializable) () -> {};
    return (T) o;
  }

  void references() {
    Supplier<List<String>> a = java.util.ArrayList<String>::new;
    Function<Integer, int[]> b = int[]::new;
    Function<Integer, String[][]> c = String[][]::new;
    Comparator<String> d = String::compareToIgnoreCase;
    BiFunction<Integer, Integer, Integer> e = (x, y) -> x + y;
    BiFunction<Integer, Integer, Integer> f = (Integer x, Integer y) -> {
      return x * y;
    };
    Function<String, String> g = s -> s.trim();
    Supplier<Generics<T, U>> h = Generics<T, U>::new;
    Class<?> k = String[].class;
    Class<?> l = int.class;
    Object m = this.<String>identity("x");
    Runnable n = super::toString;
  }

  <V> V identity(V v) {
    return v;
  }
}
