// ABOUTME: A dispatcher that calls the handler it is given, the way ts-pattern's `.with(pattern, handler)` does.
// ABOUTME: The tests pass it callbacks that end in `return`.
export const handle = <T>(value: T, handler: (value: T) => string): string => handler(value);
