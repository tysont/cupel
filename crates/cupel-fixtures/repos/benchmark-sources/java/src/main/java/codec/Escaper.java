// ABOUTME: Escapes the quotes and backslashes of a string for a JSON document.
// ABOUTME: The benchmarks beside it measure it on several inputs.
package codec;

public final class Escaper {
    public static String escape(String text) {
        StringBuilder out = new StringBuilder(text.length());
        for (char c : text.toCharArray()) {
            if (c == '"' || c == '\\') {
                out.append('\\');
            }
            out.append(c);
        }
        return out.toString();
    }
}
