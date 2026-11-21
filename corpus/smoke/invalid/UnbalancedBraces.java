package com.example.smoke.invalid;

class UnbalancedBraces {
  void f() {
    if (true) {
      System.out.println("never closed");
  }
