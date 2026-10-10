export * as geometry from "./shapes/geometry";

export const MAX_ENTRIES: number = 100;

export let currency: string = "EUR";

export type Cents = number;

export enum Currency {
  Eur = "EUR",
  Usd = "USD",
}
