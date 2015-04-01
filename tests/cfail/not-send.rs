extern crate rc;

use rc::Rc;

fn is_send<T: Send>(_: T) {}

fn rc_is_not_send<T: Send + Sync>(rc: Rc<T>) {
    is_send(rc);
    //~^ error the trait `core::marker::Send` is not implemented for the type `rc::Rc<T>`
}

fn main() {}
