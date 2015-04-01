extern crate rc;

use rc::Rc;

fn is_sync<T: Sync>(_: T) {}

fn rc_is_not_send<T: Send + Sync>(rc: Rc<T>) {
    is_sync(rc);
    //~^ error the trait `core::marker::Sync` is not implemented for the type `rc::Rc<T>`
}

fn main() {}
