package com.example.smoke.lint;

import com.example.smoke.lint.Sibling;
import com.example.smoke.lint.*;
import java.lang.String;
import java.lang.*;
import java.lang.reflect.Method;
import java.io.File;
import java.io.File;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Function;
import sun.misc.Unsafe;
import sun.*;
import static java.lang.Math.max;
import static java.lang.Math.min;
import static java.util.Collections.*;
import static java.util.Collections.*;

/**
 * Uses {@link Collection} and {@link Map.Entry} in documentation only.
 *
 * @see Set
 */
public class Imports {
  List<String> names = new ArrayList<>();
  Method method;
  Unsafe unsafe;

  /**
   * @throws java.io.IOException never
   * @param x see {@link Function#apply(Object)}
   */
  int largest(int x) {
    return max(x, 1);
  }
}
