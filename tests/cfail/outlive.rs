extern crate rc;

use rc::Rc;

fn main() {
    let rc_fn = {
        let i = 0;
        let boxed_fn: Box<Fn() -> i32> = Box::new(|| { i });
        //~^ error `i` does not live long enough

        Rc::from(boxed_fn)
    };
}
