use std::{marker::PhantomData, sync::Arc};

trait Identity<T>: Sized {
    fn from_same(this: T) -> Self;
    fn into_same(self) -> T;
}
impl<T: Sized> Identity<T> for T {
    fn from_same(this: T) -> Self { this }
    fn into_same(self) -> T { self }
}

#[macro_export]
macro_rules! mdo {
    ($i:ident <- $e:expr;) => {
        $e.bind(move |_| Free::Pure(()))
    };
    (let $i:ident = $e:expr; $($rest:tt)*) => {
        {
            let $i = $e;
            mdo!($($rest)*)
        }
    };
    ($i:ident <- $e:expr; $($rest:tt)*) => {
        $e.bind(move |$i|mdo!($($rest)*))
    };
    ($e:expr;) => {
        $e.bind(move |_| Free::Pure(()))
    };
    ($e:expr; $($rest:tt)+) => {
        $e.bind(move |_|mdo!($($rest)*))
    };
    ($e:expr) => { $e };
}

pub trait OwnedFunctor<'a>: Identity<Self::OwnedWrapped<'a, Self::OwnedUnwrapped>> {
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
pub trait OwnedApplicative<'a>: OwnedFunctor<'a> {
    fn ap<'c, F, B>(self, f: Self::OwnedWrapped<'a, F>) -> Self::OwnedWrapped<'c, B>
    where
        'a: 'c,
        B: 'a,
        F: 'a + Fn(Self::OwnedUnwrapped) -> B + Clone;
    fn pure(v: Self::OwnedUnwrapped) -> Self::OwnedWrapped<'a, Self::OwnedUnwrapped>;
}
pub trait OwnedMonad<'a>: OwnedApplicative<'a> {
    fn bind<F, B>(self, f: F) -> Self::OwnedWrapped<'a, B>
    where
        B: 'a,
        F: 'a + Fn(Self::OwnedUnwrapped) -> Self::OwnedWrapped<'a, B> + Clone;
}
pub trait MonadFree<'a>: OwnedMonad<'a> {
    type Base: OwnedFunctor<'a>;
    fn wrap<A>(
        a: <Self::Base as OwnedFunctor<'a>>::OwnedWrapped<'a, Self::OwnedWrapped<'a, A>>,
    ) -> Self::OwnedWrapped<'a, A>;
}
pub enum Free<'a, F: OwnedFunctor<'a> + 'a + Sized, A: 'a> {
    Pure(A),
    Free(Box<F::OwnedWrapped<'a, Free<'a, F, A>>>),
}
pub trait CloneWrapped<'a>: OwnedFunctor<'a> {
    fn clone_wrapped<'c, T>(wrapped: &Self::OwnedWrapped<'c, T>) -> Self::OwnedWrapped<'c, T>
    where
        'a: 'c,
        T: Clone + 'a;
}
impl<'a, F, A> Clone for Free<'a, F, A>
where
    F: OwnedFunctor<'a> + 'a + Sized + CloneWrapped<'a>,
    A: Clone + 'a,
{
    fn clone(&self) -> Self {
        match self {
            Free::Pure(a) => Free::Pure(a.clone()),
            Free::Free(f) => Free::Free(Box::new(F::clone_wrapped(&f))),
        }
    }
}
impl<'a, F: OwnedFunctor<'a, OwnedUnwrapped = A, OwnedWrapped<'a, A> = F> + 'a, A: 'a>
    OwnedFunctor<'a> for Free<'a, F, A>
{
    type OwnedUnwrapped = A;

    type OwnedWrapped<'c, T>
        = Free<'c, F::OwnedWrapped<'c, T>, T>
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
            Free::Pure(a) => Free::<'c, F::OwnedWrapped<'c, B>, B>::Pure(f(a)),
            Free::Free(fa) => {
                let mapped = fa.fmap(move |x| x.fmap(f.clone()));
                Free::Free(unsafe { Box::from_raw(Box::into_raw(Box::new(mapped)) as *mut _) })
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
    OwnedApplicative<'a> for Free<'a, F, A>
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
            Free::Pure(a) => match self {
                Free::Pure(b) => Free::Pure(a(b)),
                Free::Free(mb) => {
                    let mapped = Box::new(mb.fmap(move |x| x.fmap(a.clone())));
                    Free::Free(unsafe { Box::from_raw(Box::into_raw(mapped) as *mut _) })
                }
            },
            Free::Free(ma) => {
                let mapped = Box::new(ma.fmap(move |x| self.clone().ap(x)));
                Free::Free(unsafe { Box::from_raw(Box::into_raw(mapped) as *mut _) })
            }
        }
    }

    fn pure(v: Self::OwnedUnwrapped) -> Self::OwnedWrapped<'a, Self::OwnedUnwrapped> {
        Self::Pure(v)
    }
}
impl<'a, F: OwnedFunctor<'a, OwnedUnwrapped = A, OwnedWrapped<'a, A> = F> + 'a, A: 'a>
    OwnedMonad<'a> for Free<'a, F, A>
where
    Self: Clone,
{
    fn bind<F1, B>(self, f: F1) -> Self::OwnedWrapped<'a, B>
    where
        B: 'a,
        F1: 'a + Fn(Self::OwnedUnwrapped) -> Self::OwnedWrapped<'a, B> + Clone,
    {
        match self {
            Free::Pure(a) => f(a),
            Free::Free(m) => {
                let mapped = Box::new(m.fmap(move |x| x.bind(f.clone())));
                Free::Free(unsafe { Box::from_raw(Box::into_raw(mapped) as *mut _) })
            }
        }
    }
}
impl<'a, F: OwnedFunctor<'a, OwnedUnwrapped = T, OwnedWrapped<'a, T> = F> + 'a, T: 'a> MonadFree<'a>
    for Free<'a, F, T>
where
    Self: Clone,
{
    type Base = F::OwnedWrapped<'a, T>;

    fn wrap<A>(
        a: <Self::Base as OwnedFunctor<'a>>::OwnedWrapped<'a, Self::OwnedWrapped<'a, A>>,
    ) -> Self::OwnedWrapped<'a, A> {
        Free::Free(unsafe { Box::from_raw(Box::into_raw(Box::new(a)) as *mut _) })
    }
}

pub fn lift_f<
    'a,
    F: OwnedFunctor<'a, OwnedUnwrapped = A, OwnedWrapped<'a, A> = F>,
    A: Clone,
    M: 'a + MonadFree<'a, Base = F, OwnedUnwrapped = A>,
>(
    v: F::OwnedWrapped<'a, A>,
) -> M::OwnedWrapped<'a, A> {
    M::wrap(v.fmap(|a| M::pure(a)))
}

pub fn retract<'a, F: OwnedMonad<'a, OwnedUnwrapped = A, OwnedWrapped<'a, A> = F>, A: 'a>(
    v: Free<'a, F::OwnedWrapped<'a, A>, A>,
) -> F::OwnedWrapped<'a, A>
where
    F::OwnedWrapped<'a, Free<'a, F, A>>: OwnedMonad<'a, OwnedWrapped<'a, A> = F>,
{
    match v {
        Free::Pure(a) => F::pure(a),
        Free::Free(a) => (*a).bind::<_, A>(retract::<F, A>),
    }
}
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

// trait FTFunctionCons<'a, F: OwnedFunctor<'a>, M: OwnedMonad<'a>, A: 'a, R : 'a> 
//     where <M as OwnedFunctor<'a>>::OwnedWrapped<'a, R>: 'a,
// {
//     type FreeBind : Fn(F::OwnedWrapped<'a, M::OwnedWrapped<'a, R>>) -> M::OwnedWrapped<'a, R>;
//     type PureBind : Fn(A) -> M::OwnedWrapped<'a, R>;
// }
// impl<'a, F: OwnedFunctor<'a>, M: OwnedMonad<'a>, A: 'a, R: 'a, PureBind, FreeBind, RunFT> FTFunctionCons<'a, F, M, A, R> for FT<'a, F, M, A, R>
// where
//     FreeBind: Fn(F::OwnedWrapped<'a, M::OwnedWrapped<'a, R>>) -> M::OwnedWrapped<'a, R>,
//     PureBind: Fn(A) -> M::OwnedWrapped<'a, R>,
//     <M as OwnedFunctor<'a>>::OwnedWrapped<'a, R>: 'a,
//     RunFT: Fn(PureBind, FreeBind) -> M::OwnedWrapped<'a, R>{
//         type FreeBind = PureBind;
    
//         type PureBind = FreeBind;
//     }
// struct FT<'a, F: OwnedFunctor<'a>, M: OwnedMonad<'a>, A: 'a, R: 'a>
// {
//     run_ft: RunFT,
//     _marker: PhantomData<(&'a i32, F, M, A, R, PureBind, FreeBind)>,
// }   
