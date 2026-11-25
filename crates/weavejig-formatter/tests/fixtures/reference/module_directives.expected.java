/** Doc. */
@Deprecated
open module com.example.foo {
  requires static transitive org.apiguardian.api;
  requires static transitive org.jspecify;
  requires transitive org.junit.platform.commons;
  requires java.sql;

  exports com.example.a;
  exports com.example.b to
      x.y,
      z;

  opens com.example.b;
  opens com.example.c to
      some.very.lengthy.module.name.that.goes.on,
      another.very.lengthy.module.name.that.goes.on.and.on;

  uses com.example.Spi;
  // comment
  uses com.example.Spi2;

  provides org.junit.platform.launcher.TestExecutionListener with
      org.junit.platform.reporting.open.xml.OpenTestReportGeneratingListener;
  provides com.example.Spi with
      com.example.Impl,
      com.example.Impl2;
  provides org.opentest4j.reporting.tooling.spi.htmlreport.Contributor with
      org.junit.platform.reporting.open.xml.JUnitContributor,
      org.junit.platform.reporting.open.xml.Other;

  requires java.desktop;
}
