use super::domain::money::Cents;
use super::domain::Order;
use super::infra::Store;

pub trait Port {
    fn place(&self, order: Order, store: Store, total: Cents);
}
