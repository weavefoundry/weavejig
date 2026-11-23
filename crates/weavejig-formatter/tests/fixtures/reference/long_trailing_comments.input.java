class LongTrailingComments {
  // A trailing comment that runs past column 100 continues on `//` lines
  // that start in its column. The code before it still wraps to make room
  // for the whole comment.
  void statements() {
    int x = 1; // this is a pretty long trailing comment that will certainly run past the column limit of one hundred
    bar(aaaa); // this is a pretty long trailing comment that will certainly run past the col     limit of one hundred
    bar(aaaa); //no space after slashes and a very long comment text that goes past the column limit of one hundred
    bar(aaaa); //// four slashes and a pretty long trailing comment that will certainly run past the column limit
    bar(aaaa); // éééééé éééééé a pretty long trailing comment that will certainly run past the column limit of xx
    foo(aaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbb, cccccccccccccccccccc); // a comment that makes this line too long overall
  }

  void unbreakable() {
    // A comment with no space to break at is left as it is, and so is a
    // word that does not fit on a continuation line, with all that follows.
    int y = 2; // https://example.com/a/very/long/url/without/any/spaces/at/all/that/goes/on/and/on/and/on/forever
    int z = 2; // short words then https://example.com/a/very/long/url/without/any/spaces/at/all/that/goes/on/and/on
    bar(aaaa); // this is a pretty long trailing comment that will certainly run past the co https://example.com/a/very/long/url/without/any/spaces/at/all/that/goes/on/and/on/and/on/forever and more words
  }

  void arguments() {
    foo(a, // a trailing comment inside an argument list that is long enough to go past the limit of the line
        b);
    if (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) { // and a long comment after the brace
      baz();
    }
  }
}
