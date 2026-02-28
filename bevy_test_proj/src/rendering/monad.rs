use std::sync::Arc;

use bevy::ecs::world::Ref;

trait Identity<T>: Sized {
    fn from_same(this: T) -> Self;
    fn into_same(self) -> T;
}
impl<T: Sized> Identity<T> for T {
    fn from_same(this: T) -> Self {
        this
    }
    fn into_same(self) -> T {
        self
    }
}

macro_rules! mdo {
    ($i:ident <- $e:expr; $($rest:tt)*) => {
        $e.bind(&|$i|mdo!($($rest)*))
    };
    ($e:expr; $($rest:tt)*) => {
        $e.bind(|_|mdo!($($rest)*))
    };
    ($e:expr) => { $e };
}

trait OwnedFunctor<'a>: Identity<Self::OwnedWrapped<'a, Self::OwnedUnwrapped>> {
    type OwnedUnwrapped: 'a;
    type OwnedWrapped<'c, T>: OwnedFunctor<
        'c,
        OwnedUnwrapped = T,
        OwnedWrapped<'c, T> = Self::OwnedWrapped<'c, T>,
    >
    where
        'a: 'c,
        T: 'a;
    fn fmap<'c, B, F>(self, f: F) -> Self::OwnedWrapped<'c, B>
    where
        'a: 'c,
        B: 'a,
        F: 'a + Fn(Self::OwnedUnwrapped) -> B + Clone;
    fn cast<'c, X, Y>(
        mapped: <Self::OwnedWrapped<'c, X> as OwnedFunctor<'c>>::OwnedWrapped<'c, Y>,
    ) -> Self::OwnedWrapped<'c, Y>;
}
trait OwnedApplicative<'a>: OwnedFunctor<'a> {
    fn ap<'c, F, B>(self, f: Self::OwnedWrapped<'a, F>) -> Self::OwnedWrapped<'c, B>
    where
        'a: 'c,
        B: 'a,
        F: 'a + Fn(Self::OwnedUnwrapped) -> B + Clone;
    fn pure(v: Self::OwnedUnwrapped) -> Self::OwnedWrapped<'a, Self::OwnedUnwrapped>;
}
trait OwnedMonad<'a>: OwnedApplicative<'a> {
    fn bind<F, B>(self, f: F) -> Self::OwnedWrapped<'a, B>
    where
        B: 'a,
        F: 'a + Fn(Self::OwnedUnwrapped) -> Self::OwnedWrapped<'a, B> + Clone;
}
#[derive(Clone)]
enum FreeOwned<'a, F: OwnedFunctor<'a> + 'a + Sized, A: 'a> {
    Pure(A),
    Free(Box<F::OwnedWrapped<'a, FreeOwned<'a, F, A>>>),
}
impl<'a, F: OwnedFunctor<'a, OwnedUnwrapped = A, OwnedWrapped<'a, A> = F> + 'a, A: 'a>
    OwnedFunctor<'a> for FreeOwned<'a, F, A>
{
    type OwnedUnwrapped = A;

    type OwnedWrapped<'c, T>
        = FreeOwned<'c, F::OwnedWrapped<'c, T>, T>
    where
        'a: 'c,
        T: 'a;
    fn fmap<'c, B, F1>(self, f: F1) -> Self::OwnedWrapped<'c, B>
    where
        'a: 'c,
        B: 'a,
        F1: 'a + Fn(Self::OwnedUnwrapped) -> B + Clone,
    {
        match self {
            FreeOwned::Pure(a) => FreeOwned::<'c, F::OwnedWrapped<'c, B>, B>::Pure(f(a)),
            FreeOwned::Free(fa) => {
                let mapped = fa.fmap(move |x| x.fmap(f.clone()));
                FreeOwned::Free(unsafe { Box::from_raw(Box::into_raw(Box::new(mapped)) as *mut _) })
            }
        }
    }

    fn cast<'c, X, Y>(
        mapped: <Self::OwnedWrapped<'c, X> as OwnedFunctor<'c>>::OwnedWrapped<'c, Y>,
    ) -> Self::OwnedWrapped<'c, Y> {
        todo!()
    }
}
impl<'a, F: OwnedFunctor<'a, OwnedUnwrapped = A, OwnedWrapped<'a, A> = F> + 'a, A: 'a>
    OwnedApplicative<'a> for FreeOwned<'a, F, A>
where
    Self: Clone,
{
    fn ap<'c, F1, B>(self, f: Self::OwnedWrapped<'a, F1>) -> Self::OwnedWrapped<'c, B>
    where
        'a: 'c,
        B: 'a,
        F1: 'a + Fn(Self::OwnedUnwrapped) -> B + Clone,
    {
        match f {
            FreeOwned::Pure(a) => match self {
                FreeOwned::Pure(b) => FreeOwned::Pure(a(b)),
                FreeOwned::Free(mb) => {
                    let mapped = Box::new(mb.fmap(move |x| x.fmap(a.clone())));
                    FreeOwned::Free(unsafe { Box::from_raw(Box::into_raw(mapped) as *mut _) })
                }
            },
            FreeOwned::Free(ma) => {
                let mapped = Box::new(ma.fmap(move |x| self.clone().ap(x)));
                FreeOwned::Free(unsafe { Box::from_raw(Box::into_raw(mapped) as *mut _) })
            }
        }
    }

    fn pure(v: Self::OwnedUnwrapped) -> Self::OwnedWrapped<'a, Self::OwnedUnwrapped> {
        Self::Pure(v)
    }
}
impl<'a, F: OwnedFunctor<'a, OwnedUnwrapped = A, OwnedWrapped<'a, A> = F> + 'a, A: 'a> OwnedMonad<'a>
    for FreeOwned<'a, F, A>
where
    Self: Clone,
{
    fn bind<F1, B>(self, f: F1) -> Self::OwnedWrapped<'a, B>
    where
        B: 'a,
        F1: 'a + Fn(Self::OwnedUnwrapped) -> Self::OwnedWrapped<'a, B> + Clone,
    {
        match self {
            FreeOwned::Pure(a) => f(a),
            FreeOwned::Free(m) => { 
                let mapped = Box::new(m.fmap(move |x| x.bind(f.clone())));
                FreeOwned::Free(unsafe { Box::from_raw(Box::into_raw(mapped) as *mut _) })
            }
        }
    }
}

// fn lift_f<F: Functor, A: Clone, M: MonadFree<Base = F, Unwrapped = A>>(
//     v: F::Wrapped<A>,
// ) -> M::Wrapped<A> {
//     M::wrap(F::cast(v.fmap_consume(|a| M::pure(a))))
// }
trait RefFunctor<'a>: Identity<Self::RefWrapped<'a, Self::RefUnwrapped>>
where
    Self: 'a,
{
    type RefUnwrapped: 'a;
    type RefWrapped<'c, T>: RefFunctor<
        'c,
        RefUnwrapped = T,
        RefWrapped<'c, T> = Self::RefWrapped<'c, T>,
    >
    where
        Self: 'c,
        'a: 'c,
        T: 'a;
    fn fmap<'c, B, F>(&'a self, f: F) -> Self::RefWrapped<'a, B>
    where
        'a: 'c,
        B: 'a,
        F: 'a + Fn(&Self::RefUnwrapped) -> B + Clone;
}
enum FreeShared<'a, F: RefFunctor<'a> + 'a + Sized, A: 'a> {
    Pure(A),
    Free(Arc<F::RefWrapped<'a, FreeShared<'a, F, A>>>),
}
// impl<'a, F: RefFunctor<'a, RefUnwrapped = A, RefWrapped<'a, A> = F> + 'a, A: 'a> RefFunctor<'a>
//     for FreeShared<'a, F, A>
// {
//     type RefUnwrapped = A;

//     type RefWrapped<'c, T>
//         = FreeShared<'c, F::RefWrapped<'c, T>, T>
//     where
//         'a: 'c,
//         T: 'a;

//     fn fmap<'c, B, F1>(&'c self, f: F1) -> Self::RefWrapped<'a, B>
//     where
//         'a: 'c,
//         B: 'a,
//         F1: 'a + Fn(&Self::RefUnwrapped) -> B + Clone,
//     {
//         match self {
//             FreeShared::Pure(a) => FreeShared::Pure(f(a)),
//             FreeShared::Free(fa) => {
//                 let mapped = fa.fmap(move |x| x.fmap(f.clone()));
//                 FreeShared::Free(unsafe {
//                     Arc::from_raw(Arc::into_raw(Arc::new(mapped)) as *mut _)
//                 })
//             }
//         }
//     }
// }

enum TestDsl<'a, T> {
    ReadInt(Box<dyn Fn(i32) -> T + 'a>),
    PrintInt(i32, T),
}
impl<'b, T> OwnedFunctor<'b> for TestDsl<'b, T>
where
    T: 'b,
{
    type OwnedUnwrapped = T;

    type OwnedWrapped<'c, T1>
        = TestDsl<'c, T1>
    where
        'b: 'c,
        T1: 'b;

    fn fmap<'c, B: 'b, F>(self, f: F) -> Self::OwnedWrapped<'c, B>
    where
        'b: 'c,
        F: 'c + Fn(Self::OwnedUnwrapped) -> B,
    {
        match self {
            TestDsl::ReadInt(n_f) => TestDsl::ReadInt(Box::new(move |x| f(n_f(x)))),
            TestDsl::PrintInt(n, t) => TestDsl::PrintInt(n, f(t)),
        }
    }

    fn cast<'c, X: 'b, Y: 'b>(
        mapped: <Self::OwnedWrapped<'c, X> as OwnedFunctor<'c>>::OwnedWrapped<'c, Y>,
    ) -> Self::OwnedWrapped<'c, Y>
    where
        'b: 'c,
    {
        mapped
    }
}
impl<'b, T> RefFunctor<'b> for TestDsl<'b, T>
where
    T: 'b,
{
    type RefUnwrapped = T;

    type RefWrapped<'c, T1>
        = TestDsl<'c, T1>
    where
        'b: 'c,
        T1: 'b;

    fn fmap<'c, B, F>(&'b self, f: F) -> Self::RefWrapped<'b, B>
    where
        'b: 'c,
        B: 'b,
        F: 'b + Fn(&Self::RefUnwrapped) -> B + Clone,
    {
        match self {
            TestDsl::ReadInt(n_f) => TestDsl::ReadInt(Box::new(move |x| f(&n_f(x)))),
            TestDsl::PrintInt(n, t) => TestDsl::PrintInt(*n, f(t)),
        }
    }
}

fn read_int_from_input() {}

#[test]
fn test_dsl() {}
