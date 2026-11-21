/*
 * A small, conventionally formatted class.
 */
package com.example.smoke;

import static java.util.Objects.requireNonNull;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/** Counts words. */
public final class Basics {
  private static final int LIMIT = 100;

  private final List<String> words = new ArrayList<>();
  private int count;

  public Basics(List<String> initial) {
    requireNonNull(initial);
    this.words.addAll(initial);
    this.count = initial.size();
  }

  public int count() {
    return count;
  }

  public void add(String word) {
    if (word == null || word.isEmpty()) {
      throw new IllegalArgumentException("word must not be empty");
    }
    if (count >= LIMIT) {
      return;
    }
    words.add(word);
    count++;
  }

  public Map<String, Integer> lengths() {
    Map<String, Integer> result = new java.util.HashMap<>();
    for (String word : words) {
      result.put(word, word.length());
    }
    for (int i = 0; i < words.size(); i++) {
      // Skip the short ones.
      if (words.get(i).length() < 3) {
        continue;
      }
      result.merge(words.get(i), 1, Integer::sum);
    }
    return result;
  }

  @Override
  public String toString() {
    StringBuilder sb = new StringBuilder();
    int i = 0;
    while (i < words.size()) {
      sb.append(words.get(i)).append(i == words.size() - 1 ? "" : ", ");
      i++;
    }
    return "Basics[" + sb + "]";
  }
}
