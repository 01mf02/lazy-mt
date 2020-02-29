//! Multi-threaded lazy evaluation.
//!
//! For an introduction to lazy evaluation,
//! please see the documentation of the `lazy-st` crate.

use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::RwLock;

pub use lazy_st::Evaluate;

use self::Inner::{Evaluating, Unevaluated, Value};

/// A lazily evaluated value.
pub struct Thunk<E, V>(RwLock<Inner<E, V>>);

/// A lazily evaluated value produced from a closure.
pub type Lazy<T> = Thunk<Box<dyn FnOnce() -> T>, T>;

/// Construct a lazily evaluated value using a closure.
///
/// ~~~
/// # use lazy_mt::lazy;
/// let val = lazy!(7);
/// assert_eq!(*val, 7);
/// ~~~
#[macro_export]
macro_rules! lazy {
    ($e:expr) => {
        $crate::Thunk::new(Box::new(move || $e))
    };
}

impl<E, V> Thunk<E, V>
where
    E: Evaluate<V>,
{
    /// Create a lazily evaluated value from
    /// a value implementing the `Evaluate` trait.
    ///
    /// The `lazy!` macro is preferred if you want to
    /// construct values from closures.
    ///
    /// ~~~
    /// # use lazy_mt::Thunk;
    /// # use std::sync::Arc;
    /// # use std::thread;
    /// let expensive = Thunk::new(|| { println!("Evaluated!"); 7 });
    /// let reff = Arc::new(expensive);
    /// let reff_clone = reff.clone();
    ///
    /// // "Evaluated!" is printed below this line.
    /// thread::spawn(move || {
    ///     assert_eq!(**reff_clone, 7);
    /// });
    /// assert_eq!(**reff, 7);
    /// ~~~
    pub fn new(e: E) -> Thunk<E, V> {
        Thunk(RwLock::new(Unevaluated(e)))
    }

    /// Create a new, evaluated, thunk from a value.
    ///
    /// ~~~
    /// # use lazy_mt::{Thunk, Lazy};
    /// let x: Lazy<u32> = Thunk::evaluated(10);
    /// assert_eq!(*x, 10);
    /// ~~~
    pub fn evaluated(val: V) -> Thunk<E, V> {
        Thunk(RwLock::new(Value(val)))
    }

    /// Force evaluation of a thunk.
    pub fn force(&self) {
        if let Value(_) = *self.0.read().unwrap() {
            return;
        };

        let mut w = self.0.write().unwrap();
        // We are the thread responsible for doing the evaluation.
        match mem::replace(&mut *w, Evaluating) {
            Unevaluated(e) => *w = Value(e.evaluate()),
            Value(v) => *w = Value(v),
            _ => unreachable!(),
        }
    }
}

impl<E, V: Send + Sync> DerefMut for Thunk<E, V>
where
    E: Evaluate<V>,
{
    fn deref_mut(&mut self) -> &mut V {
        self.force();
        match *self.0.write().unwrap() {
            // Safe because getting this &'a mut T requires &'a mut self.
            Value(ref mut val) => unsafe { mem::transmute(val) },

            // We just forced this thunk.
            _ => unreachable!(),
        }
    }
}

impl<E, V: Send + Sync> Deref for Thunk<E, V>
where
    E: Evaluate<V>,
{
    type Target = V;

    fn deref(&self) -> &V {
        self.force();
        match *self.0.read().unwrap() {
            // Safe because getting this &'a T requires &'a self.
            Value(ref val) => unsafe { mem::transmute(val) },

            // We just forced this thunk.
            _ => unreachable!(),
        }
    }
}

enum Inner<E, V> {
    Unevaluated(E),
    Evaluating,
    Value(V),
}
