use std::{marker::PhantomData, sync::Arc};

use crate::rendering::monad::{CloneWrapped, Free, OwnedApplicative, OwnedFunctor, OwnedMonad, lift_f};

#[derive(Clone, Copy)]
pub struct Var<T> {
    ident: i32,
    _marker: PhantomData<T>,
}

// shuffle .xyz
// if else -> if_else(condition, Statements, Statements)
// iteration to for
// while -> while_(\break -> (condition, {})) -> While(Condition, Statements)
// call member function -> Access(Var, Function)
// call -> Call
// set -> Set

enum ShaderDslF<'a, T> {
    ReadInt(Arc<dyn Fn(i32) -> T + 'a>),
    PrintInt(i32, T),
    BeginScope(T),
    EndScope(T),
    If(Var<bool>, T),
    Else(T),
}
impl<'a, T> Clone for ShaderDslF<'a, T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::ReadInt(arg0) => Self::ReadInt(arg0.clone()),
            Self::PrintInt(arg0, arg1) => Self::PrintInt(arg0.clone(), arg1.clone()),
            Self::BeginScope(arg0) => Self::BeginScope(arg0.clone()),
            Self::EndScope(arg0) => Self::EndScope(arg0.clone()),
            Self::If(arg0, arg1) => Self::If(arg0.clone(), arg1.clone()),
            Self::Else(arg0) => Self::Else(arg0.clone()),
        }
    }
}
impl<'b, T> OwnedFunctor<'b> for ShaderDslF<'b, T>
where
    T: 'b,
{
    type OwnedUnwrapped = T;

    type OwnedWrapped<'c, T1>
        = ShaderDslF<'c, T1>
    where
        'b: 'c,
        T1: 'b;

    fn fmap<'c, B: 'b, F>(self, f: F) -> Self::OwnedWrapped<'c, B>
    where
        'b: 'c,
        F: 'c + Fn(Self::OwnedUnwrapped) -> B,
    {
        match self {
            ShaderDslF::ReadInt(n_f) => ShaderDslF::ReadInt(Arc::new(move |x| f(n_f(x)))),
            ShaderDslF::PrintInt(n, t) => ShaderDslF::PrintInt(n, f(t)),
            ShaderDslF::BeginScope(t) => ShaderDslF::BeginScope(f(t)),
            ShaderDslF::EndScope(t) => ShaderDslF::EndScope(f(t)),
            ShaderDslF::If(var, t) => ShaderDslF::If(var, f(t)),
            ShaderDslF::Else(t) => ShaderDslF::Else(f(t)),
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
// impl<'b, T> RefFunctor<'b> for ShaderDsl<'b, T>
// where
//     T: 'b,
// {
//     type RefUnwrapped = T;

//     type RefWrapped<'c, T1>
//         = ShaderDsl<'c, T1>
//     where
//         'b: 'c,
//         T1: 'b;

//     fn fmap<'c, B, F>(&'b self, f: F) -> Self::RefWrapped<'b, B>
//     where
//         'b: 'c,
//         B: 'b,
//         F: 'b + Fn(&Self::RefUnwrapped) -> B + Clone,
//     {
//         match self {
//             ShaderDsl::ReadInt(n_f) => ShaderDsl::ReadInt(Arc::new(move |x| f(&n_f(x)))),
//             ShaderDsl::PrintInt(n, t) => ShaderDsl::PrintInt(*n, f(t)),
//         }
//     }
// }
impl<'b, T> CloneWrapped<'b> for ShaderDslF<'b, T>
where
    T: 'b,
{
    fn clone_wrapped<'c, T1>(wrapped: &Self::OwnedWrapped<'c, T1>) -> Self::OwnedWrapped<'c, T1>
    where
        'b: 'c,
        T1: Clone + 'b,
    {
        wrapped.clone()
    }
}

type ShaderDSL<'a, T> = Free<'a, ShaderDslF<'a, T>, T>;
fn read_int_from_input<'a>() -> ShaderDSL<'a, i32> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDslF::ReadInt(Arc::new(|x| x)))
}
fn print_tele<'a>(v: i32) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDslF::PrintInt(v, ()))
}
fn _begin_scope<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDslF::BeginScope(()))
}
fn _end_scope<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDslF::EndScope(()))
}
fn _if_statement<'a>(cond: Var<bool>) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDslF::If(cond, ()))
}
fn _else_statement<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDslF::Else(()))
}

#[derive(Clone)]
struct IfBuilder<'a, T: Clone> {
    condition: Var<bool>,
    true_branch: ShaderDSL<'a, T>,
}
fn if_<'a, T: Clone>(cond: Var<bool>, true_statements: ShaderDSL<'a, T>) -> IfBuilder<'a, T> {
    IfBuilder {
        condition: cond,
        true_branch: true_statements,
    }
}
macro_rules! _mdo_move {

    ([$($c:ident),*] $i:ident <- $e:expr; $($rest:tt)*) => {
        $e.bind({
            // Inject clones BEFORE the closure is created
            $( let $c = $c.clone(); )*
            move |$i| _mdo_move!([$($c),*] $($rest)*)
        })
    };

    ([$($c:ident),*] $e:expr; $($rest:tt)*) => {
        $e.bind({
            // Inject clones BEFORE the closure is created
            $( let $c = $c.clone(); )*
            move |_| _mdo_move!([$($c),*] $($rest)*)
        })
    };

    ([$($c:ident),*] $e:expr) => {
        $e
    };

    ($i:ident <- $e:expr; $($rest:tt)*) => {
        $e.bind(move |$i| _mdo_move!($($rest)*))
    };

    ($e:expr; $($rest:tt)*) => {
        $e.bind(move |_| _mdo_move!($($rest)*))
    };

    ($e:expr) => { 
        $e 
    };
}
impl<'a, T: Clone> IfBuilder<'a, T> {
    pub fn else_(self, false_statements: ShaderDSL<'a, T>) -> ShaderDSL<'a, ()> {
        let s1 = Arc::new(false_statements);
        let s2 = Arc::new(self.true_branch);
        _mdo_move!{
            [s1, s2]
            _if_statement(self.condition);
            _begin_scope();
            (*s1).clone();
            _end_scope();
            _else_statement();
            _begin_scope();
            (*s2).clone();
            _end_scope()
        }
    }
    pub fn bind<F>(self, f: F) -> ShaderDSL<'a, ()>
    where
        F: 'a + Fn(T) -> ShaderDSL<'a, ()> + Clone,
    {
        let s = Arc::new(self.true_branch);
        _mdo_move!{
            [s, f]
            _if_statement(self.condition);
            _begin_scope();
            x <- (*s).clone();
            f(x);
            _end_scope();
            ShaderDSL::pure(())
        }
    }
}

fn make_bool() -> Var<bool>{
    Var { ident: 0, _marker : PhantomData}
}
fn run_tele<'a, T>(v : ShaderDSL<'a, T>, str: String) -> String{
    match v {
        Free::Pure(a) => str,
        Free::Free(step) => match *step {
            ShaderDslF::ReadInt(next_f) => {
                let mut s = String::new();
                std::io::stdin().read_line(&mut s).unwrap();
                let number: i32 = s.trim().parse().expect("Please enter a valid integer");
                run_tele(next_f(number), str + "\nReadInt")
            },
            ShaderDslF::PrintInt(n, next_prog) => {
                // println!("{}", n);
                run_tele(next_prog, str + "\nPrintInt")
            },
            ShaderDslF::BeginScope(next_prog) => run_tele(next_prog, str + "\n{"),
            ShaderDslF::EndScope(next_prog) => run_tele(next_prog, str + "\n}"),
            ShaderDslF::If(var, next_prog) => run_tele(next_prog, str + "\nif"),
            ShaderDslF::Else(_) => todo!(),
        },
    }
}
#[test]
fn test_dsl() {
    let program = mdo! {
        val <- read_int_from_input();
        print_tele(val);
        if_(make_bool(), mdo!{
            print_tele(val);
            print_tele(val);
            print_tele(val);
            print_tele(val);
        });
    };
    println!("{}", run_tele(program, String::from("")));
}
