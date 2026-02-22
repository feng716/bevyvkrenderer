trait Identity<T>: Sized {
    fn from_same(this: T) -> Self;
    fn into_same(self) -> T;
}
impl<T: Sized> Identity<T> for T {
    fn from_same(this: T) -> Self { this }
    fn into_same(self) -> T { self }
}
trait Functor: Identity<Self::Wrapped<Self::Unwrapped>> {
    type Unwrapped;
    type Wrapped<T>: Functor<Unwrapped = T>;
    fn fmap<B, F>(&self, f: F) -> Self::Wrapped<B>
    where
        F: Fn(&Self::Unwrapped) -> B;
    fn fmap_consume<B, F>(self, f: F) -> Self::Wrapped<B>
    where
        F: FnMut(Self::Unwrapped) -> B;
    fn cast<X, Y>(
        mapped: <<Self as Functor>::Wrapped<X> as Functor>::Wrapped<Y>,
    ) -> Self::Wrapped<Y>;
}
trait Applicative: Functor {
    fn pure(v: Self::Unwrapped) -> Self::Wrapped<Self::Unwrapped>;
    fn ap<B, F>(&self, f: &Self::Wrapped<&F>) -> Self::Wrapped<B>
    where
        F: Fn(&Self::Unwrapped) -> B;
}
trait Monad: Applicative {
    fn bind<B, F>(&self, f: &F) -> Self::Wrapped<B>
    where
        F: Fn(&Self::Unwrapped) -> Self::Wrapped<B>;
}
trait MonadFree: Monad {
    type Base: Functor;

    fn wrap<A>(
        fma: <Self::Base as Functor>::Wrapped<Self::Wrapped<A>>,
    ) -> Self::Wrapped<A>;
}

impl<A> Functor for Option<A> {
    type Unwrapped = A;
    type Wrapped<T> = Option<T>;
    fn fmap<B, F>(&self, f: F) -> Self::Wrapped<B>
    where
        F: Fn(&Self::Unwrapped) -> B,
    {
        match self {
            Some(v) => Some(f(v)),
            None => None,
        }
    }

    fn cast<X, Y>(
        mapped: <<Self as Functor>::Wrapped<X> as Functor>::Wrapped<Y>,
    ) -> Self::Wrapped<Y> {
        mapped
    }
    
    fn fmap_consume<B, F>(self, f: F) -> Option<B>
    where
        F: FnMut(A) -> B {
            self.map(f)
    }
}
impl<A> Applicative for Option<A> {
    fn ap<B, F>(&self, f: &Self::Wrapped<&F>) -> Self::Wrapped<B>
    where
        F: Fn(&Self::Unwrapped) -> B,
    {
        match f {
            Some(f) => self.fmap(f),
            None => None,
        }
    }

    fn pure(v: Self::Unwrapped) -> Self::Wrapped<Self::Unwrapped> {
        Some(v)
    }
}
impl<A> Monad for Option<A> {
    fn bind<B, F>(&self, f: &F) -> Self::Wrapped<B>
    where
        F: Fn(&Self::Unwrapped) -> Self::Wrapped<B>,
    {
        match self {
            Some(val) => f(val),
            None => None,
        }
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

enum Free<F: Functor, A> {
    Pure(A),
    Free(Box<F::Wrapped<Free<F, A>>>),
}

impl<F: Functor, A> Functor for Free<F, A> {
    type Unwrapped = A;

    type Wrapped<T> = Free<F, T>;

    fn fmap<B, F1>(&self, f: F1) -> Self::Wrapped<B>
    where
        F1: Fn(&Self::Unwrapped) -> B,
    {
        match self {
            Free::Pure(a) => Free::Pure(f(a)),
            Free::Free(fa) => {
                let mapped = (*fa).fmap(|x| x.fmap(&f));
                Free::Free(Box::new(F::cast(mapped)))
            }
        }
    }
    fn cast<X, Y>(mapped: Self::Wrapped<Y>) -> Self::Wrapped<Y> {
        mapped
    }
    
    fn fmap_consume<B, F1>(self, mut f: F1) -> Self::Wrapped<B>
    where
        F1: FnMut(Self::Unwrapped) -> B {
        match self {
            Free::Pure(a) => Free::Pure(f(a)),
            Free::Free(fa) => {
                let mapped = (*fa).fmap_consume(|x| x.fmap_consume(&mut f));
                Free::Free(Box::new(F::cast(mapped)))
            }
        }
    }
}
impl<F: Functor, A> Applicative for Free<F, A> {
    fn pure(v: A) -> Free<F, A> {
        Free::Pure(v)
    }

    fn ap<B, F1>(&self, f: &Free<F, &F1>) -> Free<F, B>
    where
        F1: Fn(&A) -> B,
    {
        match f {
            Free::Pure(a) => match self {
                Free::Pure(b) => Free::Pure(a(b)),
                Free::Free(mb) => Free::Free(Box::new(F::cast(mb.fmap(|v| v.fmap(&a))))),
            },
            Free::Free(ma) => Free::Free(Box::new(F::cast(ma.fmap(|v|self.ap(v))))),
        }
    }
}

impl<F : Functor, A> Monad for Free<F, A> {
    fn bind<B, F1>(&self, f: &F1) -> Free<F, B>
    where
        F1: Fn(&A) -> Free<F, B> {
        match self {
            Free::Pure(a) => f(a),
            Free::Free(m) => Free::Free(Box::new(F::cast(m.fmap(|v|v.bind(f))))),
        }
    }
}
impl<F1: Functor, T> MonadFree for Free<F1, T> {
    type Base = F1;

    fn wrap<A>(
        fma: F1::Wrapped<Free<F1, A>>,
    ) -> Free<F1, A> {
        Free::Free(Box::new(fma))
    }
}
fn lift_f<F: Functor, A : Clone, M : MonadFree<Base = F, Unwrapped = A>>(v : F::Wrapped<A>) -> M::Wrapped<A>{
    M::wrap(F::cast(v.fmap_consume(|a|M::pure(a))))
}

enum TestDsl<T> 
{
    ReadInt(Box<dyn Fn(i32) -> T>),
    PrintInt(i32, T)
}
impl<T> Functor for TestDsl<T>{
    type Unwrapped = T;

    type Wrapped<T1> = TestDsl<T1>;

    fn fmap<B, F>(&self, f: F) -> Self::Wrapped<B>
    where
        F: Fn(&Self::Unwrapped) -> B {
        todo!()
    }

    fn fmap_consume<B, F>(self, f: F) -> Self::Wrapped<B>
    where
        F: FnMut(Self::Unwrapped) -> B {
        todo!()
    }

    fn cast<X, Y>(
        mapped: <<Self as Functor>::Wrapped<X> as Functor>::Wrapped<Y>,
    ) -> Self::Wrapped<Y> {
        todo!()
    }
}

#[test]
fn test_dsl() {
}

#[test]
fn test_option() {
    let v = mdo! {
        a <- Some(1);
        Option::pure(())
    };
    assert_eq!(Some(String::from("1")), Some(1).fmap(|v| v.to_string()));
    assert_eq!(None, None.fmap(|v: &i32| v + 1));
}
