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

trait Applicative<'a>: Functor<'a> {
    fn pure(v: Self::Unwrapped) -> Self::Wrapped<Self::Unwrapped>;
    fn ap<B, F>(&self, f: &Self::Wrapped<&F>) -> Self::Wrapped<B>
    where
        F: Fn(&Self::Unwrapped) -> B;
}
trait Monad<'a>: Applicative<'a> {
    fn bind<B, F>(&self, f: &F) -> Self::Wrapped<B>
    where
        F: Fn(&Self::Unwrapped) -> Self::Wrapped<B>;
}
trait MonadFree<'a>: Monad<'a> {
    type Base<'b>: Functor<'b>;

    fn wrap<A>(fma: <Self::Base<'a> as Functor>::Wrapped<Self::Wrapped<A>>) -> Self::Wrapped<A>;
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

enum Free<'a, F: Functor<'a> + 'a, A: 'a> {
    Pure(A),
    Free(Box<F::Wrapped<Free<'a, F, A>>>),
}

impl<'a, F: Functor<'a> + 'a, A: 'a> Functor<'a> for Free<'a, F, A> {
    type Unwrapped = A;

    type Wrapped<T>
        = Free<'a, F, T>
    where
        T: 'a;

    fn fmap<B: 'a, F1>(&'a self, f: F1) -> Self::Wrapped<B>
    where
        F1: 'a + Fn(&Self::Unwrapped) -> B,
    {
        match self {
            Free::Pure(a) => Free::Pure(f(a)),
            Free::Free(fa) => {
                let mapped = (*fa).fmap(|x| x.fmap(&f));
                Free::Free(Box::new(F::cast(mapped)))
            }
        }
    }

    fn fmap_consume<B, F1>(self, f: F1) -> Self::Wrapped<B>
    where
        F1: 'a + Fn(Self::Unwrapped) -> B,
    {
        todo!()
    }

    fn cast<X: 'a, Y: 'a>(
        mapped: <Self::Wrapped<X> as Functor<'a>>::Wrapped<Y>,
    ) -> Self::Wrapped<Y> {
        todo!()
    }
}

impl<F1: Functor, T> MonadFree for Free<F1, T> {
    type Base = F1;

    fn wrap<A>(fma: F1::Wrapped<Free<F1, A>>) -> Free<F1, A> {
        Free::Free(Box::new(fma))
    }
}
fn lift_f<F: Functor, A: Clone, M: MonadFree<Base = F, Unwrapped = A>>(
    v: F::Wrapped<A>,
) -> M::Wrapped<A> {
    M::wrap(F::cast(v.fmap_consume(|a| M::pure(a))))
}

enum TestDsl<'a, T> {
    ReadInt(Box<dyn Fn(i32) -> T + 'a>),
    PrintInt(i32, T),
}
trait Functor<'a>: Identity<Self::Wrapped<Self::Unwrapped>> {
    type Unwrapped: 'a;
    type Wrapped<T>: Functor<'a, Unwrapped = T>
    where
        T: 'a;
    fn fmap<B: 'a, F>(&'a self, f: F) -> Self::Wrapped<B>
    where
        F: 'a + Fn(&Self::Unwrapped) -> B;
    fn fmap_consume<B, F>(self, f: F) -> Self::Wrapped<B>
    where
        F: 'a + Fn(Self::Unwrapped) -> B;
    fn cast<X, Y>(mapped: <Self::Wrapped<X> as Functor<'a>>::Wrapped<Y>) -> Self::Wrapped<Y>;
}
impl<'b, T> Functor<'b> for TestDsl<'b, T>
where
    T: 'b,
{
    type Unwrapped = T;

    type Wrapped<T1>
        = TestDsl<'b, T1>
    where
        T1: 'b;

    fn fmap<B: 'b, F>(&'b self, f: F) -> Self::Wrapped<B>
    where
        F: 'b + Fn(&Self::Unwrapped) -> B,
    {
        match self {
            TestDsl::ReadInt(n_f) => TestDsl::ReadInt(Box::new(move |x| f(&n_f(x)))),
            TestDsl::PrintInt(n, t) => TestDsl::PrintInt(*n, f(t)),
        }
    }

    fn fmap_consume<B: 'b, F>(self, f: F) -> Self::Wrapped<B>
    where
        F: 'b + Fn(Self::Unwrapped) -> B,
    {
        match self {
            TestDsl::ReadInt(n_f) => TestDsl::ReadInt(Box::new(move |x| f(n_f(x)))),
            TestDsl::PrintInt(n, t) => TestDsl::PrintInt(n, f(t)),
        }
    }

    fn cast<X: 'b, Y: 'b>(
        mapped: <Self::Wrapped<X> as Functor<'b>>::Wrapped<Y>,
    ) -> Self::Wrapped<Y> {
        mapped
    }
}
fn read_int_from_input() {}

#[test]
fn test_dsl() {}

fn test_<'a, F : Functor<'a> + 'a, A : 'a>() {
}
