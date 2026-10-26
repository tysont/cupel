// ABOUTME: A parser whose type the tests pin: a numeric string parses to a number.
// ABOUTME: Its return type is what the type-level assertions of the tests check.
export const parse = (input: string): number => Number(input);
