// ABOUTME: A type-level utility the tests check: what remains of a union once a pattern's type is removed.
// ABOUTME: It has no runtime code; its tests are its declared types.
export type Remaining<Input, Pattern> = Input extends Pattern ? never : Input;
