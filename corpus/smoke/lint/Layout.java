package com.example.smoke.lint;

public class Layout {
	int tabbed = 1;
  String wide = "this line is deliberately much longer than one hundred columns wide, to trip the limit";
	int again = 2;

  void run() {
    int value = wide.length() + tabbed + again + wide.length() + tabbed + again + wide.length() + tabbed;
  }
}