use std::sync::Arc;

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
pub enum FreeOwned<'a, F: OwnedFunctor<'a> + 'a + Sized, A: 'a> {
    Pure(A),
    Free(Box<F::OwnedWrapped<'a, FreeOwned<'a, F, A>>>),
}
trait CloneWrapped<'a>: OwnedFunctor<'a> {
    fn clone_wrapped<'c, T>(
        wrapped: &Self::OwnedWrapped<'c, T>,
    ) -> Self::OwnedWrapped<'c, T>
    where
        'a: 'c,
        T: Clone + 'a;
}
impl<'a, F, A> Clone for FreeOwned<'a, F, A>
where
    F: OwnedFunctor<'a> + 'a + Sized + CloneWrapped<'a>,
    A: Clone + 'a,
{
    fn clone(&self) -> Self {
        match self {
            FreeOwned::Pure(a) => FreeOwned::Pure(a.clone()),
            FreeOwned::Free(f) => FreeOwned::Free(Box::new(F::clone_wrapped(&f))),
        }
    }
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
impl<'a, F: OwnedFunctor<'a, OwnedUnwrapped = A, OwnedWrapped<'a, A> = F> + 'a, A: 'a>
    OwnedMonad<'a> for FreeOwned<'a, F, A>
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
impl<'a, F : OwnedFunctor<'a, OwnedUnwrapped = T, OwnedWrapped<'a, T> = F> + 'a, T : 'a> MonadFree<'a> for FreeOwned<'a, F, T>
    where Self : Clone
{
    type Base = F::OwnedWrapped<'a, T>;

    fn wrap<A>(
        a: <Self::Base as OwnedFunctor<'a>>::OwnedWrapped<'a, Self::OwnedWrapped<'a, A>>,
    ) -> Self::OwnedWrapped<'a, A> {
        FreeOwned::Free(unsafe { Box::from_raw(Box::into_raw(Box::new(a)) as *mut _)} )
    }
}

pub fn lift_f<'a, F: OwnedFunctor<'a, OwnedUnwrapped = A, OwnedWrapped<'a, A> = F>, A: Clone, M: 'a + MonadFree<'a, Base = F, OwnedUnwrapped = A>>(
    v: F::OwnedWrapped<'a, A>,
) -> M::OwnedWrapped<'a, A> {
    M::wrap(v.fmap(|a| M::pure(a)))
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

enum TestDsl<'a, T> {
    ReadInt(Arc<dyn Fn(i32) -> T + 'a>),
    PrintInt(i32, T),
}
impl<'a, T> Clone for TestDsl<'a, T>
    where T : Clone
{
    fn clone(&self) -> Self {
        match self {
            Self::ReadInt(arg0) => Self::ReadInt(arg0.clone()),
            Self::PrintInt(arg0, arg1) => Self::PrintInt(arg0.clone(), arg1.clone()),
        }
    }
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
            TestDsl::ReadInt(n_f) => TestDsl::ReadInt(Arc::new(move |x| f(n_f(x)))),
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
            TestDsl::ReadInt(n_f) => TestDsl::ReadInt(Arc::new(move |x| f(&n_f(x)))),
            TestDsl::PrintInt(n, t) => TestDsl::PrintInt(*n, f(t)),
        }
    }
}
impl<'b, T> CloneWrapped<'b> for TestDsl<'b, T>
where
    T: 'b,
{
    fn clone_wrapped<'c, T1>(
        wrapped: &Self::OwnedWrapped<'c, T1>,
    ) -> Self::OwnedWrapped<'c, T1>
    where
        'b: 'c,
        T1: Clone + 'b,
    {
        wrapped.clone()
    }
}

type TestDslF<'a, T> = FreeOwned<'a, TestDsl<'a, T>, T>;
fn read_int_from_input<'a>() -> TestDslF<'a, i32>{
    lift_f::<'_, _, _, TestDslF<'_, _>>(TestDsl::ReadInt(Arc::new(|x|x)))
}
fn print_tele<'a>(v : i32) -> TestDslF<'a, ()>{
    lift_f::<'_, _, _, TestDslF<'_, _>>(TestDsl::PrintInt(v, ()))
}
fn run_tele<'a, T>(v : TestDslF<'a, T>) {
    match v {
        FreeOwned::Pure(a) => (),
        FreeOwned::Free(step) => match *step {
            TestDsl::ReadInt(next_f) => {
                let mut s = String::new();
                std::io::stdin().read_line(&mut s).unwrap();
                let number: i32 = s.trim().parse().expect("Please enter a valid integer");
                run_tele(next_f(number));
            },
            TestDsl::PrintInt(n, next_prog) => {
                println!("{}", n);
                run_tele(next_prog);
            },
        },
    }
}

#[test]
fn test_dsl() {
    let program = mdo!{
        val <- read_int_from_input();
        print_tele(val)
    };
    run_tele(program);
    read_int_from_input();
}
