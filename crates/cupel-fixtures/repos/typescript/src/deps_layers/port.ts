import { Order } from "./domain/order";

export interface Port {
  place(order: Order): void;
}
