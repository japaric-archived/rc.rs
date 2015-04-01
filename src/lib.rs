//! A reference-counted pointer that accepts DST: `Rc<str>`, `Rc<[T]>`, `Rc<Fn>`, etc

#![cfg_attr(test, plugin(quickcheck_macros))]
#![deny(missing_docs)]
#![deny(warnings)]
#![feature(alloc)]
#![feature(collections)]
#![feature(convert)]
#![feature(core)]
#![feature(custom_attribute)]
#![feature(filling_drop)]
#![feature(optin_builtin_traits)]
#![feature(plugin)]
#![feature(unsafe_no_drop_flag)]

#[cfg(test)] extern crate quickcheck;
#[cfg(test)] extern crate rand;

extern crate alloc;
extern crate core;

use core::nonzero::NonZero;
use std::borrow::Borrow;
use std::boxed;
use std::cell::Cell;
use std::hash::{Hash, Hasher};
use std::mem;
use std::ops::Deref;

/// A reference-counted pointer type over an immutable value.
///
/// # Examples
///
/// ```
/// # #![feature(convert)]
/// # extern crate rc;
/// # use rc::Rc;
/// # fn main() {
/// let boxed_fn: Box<Fn() -> i32> = Box::new(|| { 0 });
/// //boxed_fn.clone();  //~ error: does not implement `clone`
/// let original: Rc<Fn() -> i32> = Rc::from(boxed_fn);  // moves `boxed_fn`
/// let rc_fn = original.clone();  // increases refcount to 2
///
/// drop(original);  // decreases refcount to 1
///
/// // overloaded calls work with `Rc<Fn(..) -> ..>`s
/// assert_eq!(rc_fn(), 0);
///
/// // `rc_fn` dropped here, refcount reaches zero, `boxed_fn` is deallocated
/// # }
/// ```
///
/// # Layout
///
/// A single layer of indirection.
///
/// ``` text
///    Stack       |   Heap
///                |
///    Rc<str>     |
/// +------------+ |
/// | *mut str   |-|-> "Hello, world!"
/// | *mut usize |-|-> 3
/// +------------+ |
/// ```
///
/// ^ NOTE: `Cell`/`NonZero` wrappers omitted for brevity. String and reference count are not
/// (necessarily) stored in contiguous memory.
///
/// # Size
///
/// For sized types: 2 words, for DST: 3 words.
///
/// ```
/// # extern crate rc;
/// # use std::mem;
/// # use rc::Rc;
/// # fn main() {
/// assert_eq!(mem::size_of::<Rc<()>>(),    2 * mem::size_of::<usize>());
/// assert_eq!(mem::size_of::<Rc<[i32]>>(), 3 * mem::size_of::<usize>());
/// assert_eq!(mem::size_of::<Rc<str>>(),   3 * mem::size_of::<usize>());
/// # }
/// ```
#[unsafe_no_drop_flag]
pub struct Rc<T: ?Sized> {
    /// The number of references
    count: NonZero<*mut Cell<usize>>,
    /// A pointer to the heap allocated data
    data: NonZero<*mut T>,
}

impl<T> Rc<T> {
    /// Creates a new `Rc` pointer.
    ///
    /// NOTE: `value` will be allocated in the heap. If you have a heap allocated value like `Box`,
    /// `String` or `Vec`, use the `Rc::from()` method instead.
    pub fn new(value: T) -> Rc<T> {
        Rc::from(Box::new(value))
    }
}

impl<T: ?Sized> Rc<T> {
    /// Returns the number of references to this value.
    pub fn count(&self) -> usize {
        unsafe {
            (**self.count).get()
        }
    }

    fn dec_count(&self) {
        unsafe {
            (**self.count).set(self.count() - 1)
        }
    }

    fn inc_count(&self) {
        unsafe {
            (**self.count).set(self.count() + 1)
        }
    }
}

impl<T: ?Sized> Borrow<T> for Rc<T> {
    fn borrow(&self) -> &T {
        self
    }
}

impl<T: ?Sized> Clone for Rc<T> {
    fn clone(&self) -> Rc<T> {
        self.inc_count();

        Rc {
            count: self.count,
            data: self.data,
        }
    }
}

impl<T: ?Sized> Eq for Rc<T> where T: Eq {}

impl<'a, T> From<&'a [T]> for Rc<[T]> where T: Clone {
    /// NOTE: This requires allocating the `slice` first (`Vec::to_vec`).
    fn from(slice: &[T]) -> Rc<[T]> {
        Rc::from(slice.to_vec())
    }
}

impl<'a> From<&'a str> for Rc<str> {
    /// NOTE: This requires allocating the `string` first (`String::from_str`).
    fn from(string: &str) -> Rc<str> {
        Rc::from(String::from_str(string))
    }
}

impl<T: ?Sized> From<Box<T>> for Rc<T> {
    /// NOTE: this involves a single, small heap allocation for the reference count. `boxed_value`
    /// will *not* be reallocated.
    fn from(boxed_value: Box<T>) -> Rc<T> {
        unsafe {
            Rc {
                count: NonZero::new(boxed::into_raw(Box::new(Cell::new(1)))),
                data: NonZero::new(boxed::into_raw(boxed_value)),
            }
        }
    }
}

// TODO(rust-lang/rust#18283) use `Rc::from(string.into_boxed_str())` instead of `transmute`
impl From<String> for Rc<str> {
    /// NOTE: This calls `shrink_to_fit` on `string` (on the underlying `Vec<u8>`), which may incur
    /// in a reallocation.
    fn from(string: String) -> Rc<str> {
        // Create a `Rc<[u8]>` first, and then transmute that into a `Rc<str>`
        unsafe {
            mem::transmute(Rc::from(string.into_bytes()))
        }
    }
}

impl<T> From<Vec<T>> for Rc<[T]> {
    /// NOTE: This calls `shrink_to_fit` on `vec`, which may incur in a reallocation.
    fn from(vec: Vec<T>) -> Rc<[T]> {
        Rc::from(vec.into_boxed_slice())
    }
}

impl<T: ?Sized> Deref for Rc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe {
            mem::transmute(*self.data)
        }
    }
}

impl<T: ?Sized> Drop for Rc<T> {
    fn drop(&mut self) {
        let ptr = *self.count;

        if !ptr.is_null() && ptr as usize != mem::POST_DROP_USIZE {
            unsafe {
                self.dec_count();

                if self.count() == 0 {
                    drop(Box::from_raw(*self.count));
                    drop(Box::from_raw(*self.data));
                }
            }
        }
    }
}

impl<T: ?Sized> Hash for Rc<T> where T: Hash {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        Hash::hash(&**self, state)
    }
}

impl<T: ?Sized> PartialEq for Rc<T> where T: PartialEq {
    fn eq(&self, rhs: &Rc<T>) -> bool {
        PartialEq::eq(&**self, &**rhs)
    }
}

impl<T> !Send for Rc<T> {}
impl<T> !Sync for Rc<T> {}

#[cfg(test)]
mod test {
    use rand::{Rng, XorShiftRng, self};
    use quickcheck::TestResult;

    use Rc;

    #[test]
    fn closure_borrow() {
        let i = 0;
        let rc_fn = {
            let boxed_fn: Box<Fn() -> i32> = Box::new(|| { i });
            Rc::from(boxed_fn)
        };

        assert_eq!(rc_fn(), 0);
    }

    #[test]
    fn closure_move() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert("A".to_string(), Box::new(0));

        let boxed_fn: Box<Fn(&str) -> i32> = Box::new(move |k| **map.get(k).unwrap());
        let rc_fn = Rc::from(boxed_fn);

        assert_eq!(rc_fn("A"), 0);
    }

    /// `&str` -> `Rc<str>`, where `&str` is not necessarily aligned
    #[quickcheck]
    fn from_str(len: usize, offset: usize, copies: usize) -> TestResult {
        if offset > len {
            return TestResult::discard()
        }

        // create a random string
        let string = rand::thread_rng()
            .gen::<XorShiftRng>()
            .gen_ascii_chars()
            .take(len)
            .collect::<String>();

        // slice the random string
        let str: &str = &string[offset..];
        // create a ref counted string from the slice (this allocates)
        let rcstr = Rc::from(str);

        // create multiple references
        let references = (0..copies).map(|_| rcstr.clone()).collect::<Vec<_>>();

        // drop the original reference
        drop(rcstr);

        // check data integrity
        TestResult::from_bool(references.iter().all(|rcstr| &**rcstr == str))
    }

    /// `String` -> `Rc<str>`, where `String` may have `capacity >= length`
    #[quickcheck]
    fn from_string(cap: usize, len: usize, copies: usize) -> bool {
        // initialize empty string with some reserved capacity
        let mut string = String::with_capacity(cap);

        // push random ascii chars into the string
        for c in rand::thread_rng().gen::<XorShiftRng>().gen_ascii_chars().take(len) {
            string.push(c);
        }

        // create a ref counted string (consumes `string`)
        let original = string.clone();
        let rcstr = Rc::from(string);

        // create multiple owners
        let copies = (0..copies).map(|_| rcstr.clone()).collect::<Vec<_>>();

        // drop original owner
        drop(rcstr);

        // check data integrity
        copies.iter().all(|rcstr| &**rcstr == original)
    }

    /// `Box<Fn(..) -> ..>` -> `Rc<Fn(..) -> ..>`
    #[test]
    fn rc_fn() {
        let trait_object: Box<Fn() -> i32> = Box::new(|| { 0 });
        let rc_fn = Rc::from(trait_object);

        assert_eq!(rc_fn(), 0);
    }

    #[should_panic]
    #[test]
    fn unwind() {
        let str = "Hello, world!";
        let _rc_str_0 = Rc::from(str);
        let _rc_str_1 = Rc::from(str.to_string());
        let slice: &[_] = &[0, 1, 2];
        let _rc_slice_0 = Rc::from(slice);
        let _rc_slice_1 = Rc::from(slice.to_vec());
        let boxed_fn: Box<Fn()> = Box::new(|| {});
        let _rc_fn = Rc::from(boxed_fn);

        panic!();
    }
}
