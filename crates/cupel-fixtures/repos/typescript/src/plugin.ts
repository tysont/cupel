export interface Registry {
  register(name: string, options: { version: number }): void;
}

export default function (registry: Registry): void {
  registry.register("ledger", { version: 1 });
}
