import com.sun.source.util.JavacTask;
import java.io.BufferedReader;
import java.io.File;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.List;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;

/**
 * Reference parser for the scoring harness.
 *
 * <p>Reads file paths from standard input, one per line, and parses each with javac's parser only
 * (no symbol resolution, so missing dependencies do not matter). For every file it prints {@code
 * accept} or {@code reject}, a tab, and the path. The first line of output is {@code version}, a
 * tab, and the JDK version.
 *
 * <p>Run with the source launcher: {@code java reference/JavacParse.java < paths.txt}
 */
public class JavacParse {
  public static void main(String[] args) throws Exception {
    PrintStream out = new PrintStream(System.out, false, StandardCharsets.UTF_8);
    out.println("version\t" + Runtime.version().feature() + " (" + Runtime.version() + ")");

    JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
    StandardJavaFileManager files =
        compiler.getStandardFileManager(null, null, StandardCharsets.UTF_8);
    BufferedReader in =
        new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));

    String path;
    while ((path = in.readLine()) != null) {
      if (path.isEmpty()) {
        continue;
      }
      DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
      boolean ok;
      try {
        JavacTask task =
            (JavacTask)
                compiler.getTask(
                    null,
                    files,
                    diagnostics,
                    List.of("-proc:none", "-Xlint:none"),
                    null,
                    files.getJavaFileObjects(new File(path)));
        task.parse();
        ok =
            diagnostics.getDiagnostics().stream()
                .noneMatch(d -> d.getKind() == Diagnostic.Kind.ERROR);
      } catch (RuntimeException | Error e) {
        ok = false;
      }
      out.println((ok ? "accept" : "reject") + "\t" + path);
    }
    out.flush();
  }
}
