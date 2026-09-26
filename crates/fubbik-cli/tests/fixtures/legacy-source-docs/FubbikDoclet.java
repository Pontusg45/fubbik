import java.io.IOException;
import java.nio.file.*;
import java.util.*;
import javax.lang.model.SourceVersion;
import javax.lang.model.element.*;
import jdk.javadoc.doclet.*;
import com.sun.source.doctree.*;
import com.sun.source.util.*;

/** Emits documented declarations as Fubbik symbols using the JDK comment tree. */
public class FubbikDoclet implements Doclet {
    private String output;
    private Path root;
    private Reporter reporter;
    public void init(Locale locale, Reporter reporter) { this.reporter = reporter; }
    public String getName() { return "Fubbik"; }
    public SourceVersion getSupportedSourceVersion() { return SourceVersion.latestSupported(); }
    private Option option(String name) {
        return new Option() {
            public int getArgumentCount() { return 1; }
            public String getDescription() { return name; }
            public Kind getKind() { return Kind.OTHER; }
            public List<String> getNames() { return List.of(name); }
            public String getParameters() { return "path"; }
            public boolean process(String option, List<String> args) {
                if (name.equals("-fubbik-output")) output = args.get(0);
                else root = Path.of(args.get(0)).toAbsolutePath().normalize();
                return true;
            }
        };
    }
    public Set<? extends Option> getSupportedOptions() {
        return Set.of(option("-fubbik-output"), option("-fubbik-root"));
    }
    private static String quote(String value) {
        StringBuilder s = new StringBuilder("\"");
        for (char c : value.toCharArray()) {
            switch (c) {
                case '\\': s.append("\\\\"); break;
                case '"': s.append("\\\""); break;
                case '\n': s.append("\\n"); break;
                case '\r': s.append("\\r"); break;
                case '\t': s.append("\\t"); break;
                default: if (c < 32) s.append(String.format("\\u%04x", (int)c)); else s.append(c);
            }
        }
        return s.append('"').toString();
    }
    private static String qualified(Element e) {
        if (e instanceof QualifiedNameable) return ((QualifiedNameable)e).getQualifiedName().toString();
        return qualified(e.getEnclosingElement()) + "#" + e.toString();
    }
    public boolean run(DocletEnvironment env) {
        try {
            if (root == null || output == null) throw new IOException("Missing Fubbik options");
            List<String> result = new ArrayList<>();
            Set<Element> seen = new HashSet<>();
            for (Element e : env.getIncludedElements()) visit(e, env, seen, result);
            Collections.sort(result);
            Files.writeString(Path.of(output), "[" + String.join(",", result) + "]");
            return true;
        } catch (Exception e) {
            reporter.print(javax.tools.Diagnostic.Kind.ERROR, e.toString());
            return false;
        }
    }
    private void visit(Element e, DocletEnvironment env, Set<Element> seen, List<String> result) throws IOException {
        if (!seen.add(e) || !env.isIncluded(e)) return;
        DocTrees trees = env.getDocTrees();
        DocCommentTree comment = trees.getDocCommentTree(e);
        TreePath tree = trees.getPath(e);
        if (comment != null && tree != null) {
            Path source = Path.of(tree.getCompilationUnit().getSourceFile().toUri()).toAbsolutePath().normalize();
            if (!source.startsWith(root)) throw new IOException("Source outside extraction root: " + source);
            String path = root.relativize(source).toString().replace('\\','/');
            long position = trees.getSourcePositions().getStartPosition(tree.getCompilationUnit(), tree.getLeaf());
            long line = tree.getCompilationUnit().getLineMap().getLineNumber(position);
            StringBuilder doc = new StringBuilder();
            for (DocTree part : comment.getFullBody()) doc.append(part.toString());
            for (DocTree tag : comment.getBlockTags()) doc.append("\n\n").append(tag.toString());
            String key = qualified(e);
            String signature = e.toString();
            if (e instanceof ExecutableElement) signature = ((ExecutableElement)e).getReturnType() + " " + signature;
            result.add("{\"key\":" + quote(key) + ",\"title\":" + quote(key)
                + ",\"signature\":" + quote(signature) + ",\"documentation\":" + quote(doc.toString())
                + ",\"path\":" + quote(path) + ",\"line\":" + Math.max(1,line) + ",\"references\":[]}");
        }
        for (Element child : e.getEnclosedElements()) visit(child, env, seen, result);
    }
}
