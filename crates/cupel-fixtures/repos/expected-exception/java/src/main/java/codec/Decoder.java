// ABOUTME: A decoder that rejects text outside its alphabet.
// ABOUTME: Its tests expect the exception it throws.
package codec;

public final class Decoder {
    public static int decode(String text) {
        if (!text.matches("[A-Za-z]*")) {
            throw new IllegalArgumentException("Illegal character in " + text);
        }
        return text.length();
    }
}
