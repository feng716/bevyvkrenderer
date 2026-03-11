use std::sync::Arc;

use crate::{
    _mdo_move,
    rendering::dsl::{
        monad::OwnedMonad,
        shader_dsl::{FuncArg, FuncName, ShaderDSL, Var, _call_func_rt, _new_ident},
        vec_op::Vec3,
    },
};

fn dsl_cmp_op<'a, A, B, T: 'a>(op: FuncName, a: A, b: B) -> ShaderDSL<'a, Var<bool>>
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    B: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    let a_dsl = Arc::new(a.into());
    let b_dsl = Arc::new(b.into());
    let op = Arc::new(op);

    _mdo_move! {
        [a_dsl, b_dsl, op]
        ident <- _new_ident(); // This correctly returns Var<bool> from the DSL monad context
        v1 <- (*a_dsl).clone();
        v2 <- (*b_dsl).clone();
        _call_func_rt(
            (*op).clone(),
            vec![v1.into(), v2.into()],
            ident
        )
    }
}
pub trait ShaderCmp<'a, T: 'a> {
    fn eq<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>>;
    fn neq<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>>;
    fn lt<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>>;
    fn lte<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>>;
    fn gt<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>>;
    fn gte<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>>;
}
impl<'a, A, T: 'a> ShaderCmp<'a, T> for A
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    fn eq<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>> {
        dsl_cmp_op(FuncName::Eq, self, other)
    }
    fn neq<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>> {
        dsl_cmp_op(FuncName::Neq, self, other)
    }
    fn lt<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>> {
        dsl_cmp_op(FuncName::Lt, self, other)
    }
    fn lte<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>> {
        dsl_cmp_op(FuncName::Lte, self, other)
    }
    fn gt<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>> {
        dsl_cmp_op(FuncName::Gt, self, other)
    }
    fn gte<B: Into<ShaderDSL<'a, Var<T>>>>(self, other: B) -> ShaderDSL<'a, Var<bool>> {
        dsl_cmp_op(FuncName::Gte, self, other)
    }
}

pub fn dsl_builtin_1<'a, A, InputT: 'a, RetT>(op: FuncName, a: A) -> ShaderDSL<'a, Var<RetT>>
where
    A: Into<ShaderDSL<'a, Var<InputT>>>,
    Var<InputT>: Into<FuncArg>,
{
    let a_dsl = Arc::new(a.into());
    let op = Arc::new(op);
    _mdo_move! {
        [a_dsl, op]
        ident <- _new_ident();
        v1 <- (*a_dsl).clone();
        _call_func_rt((*op).clone(), vec![v1.into()], ident)
    }
}

pub fn dsl_builtin_2<'a, A, B, InputT: 'a, RetT>(
    op: FuncName,
    a: A,
    b: B,
) -> ShaderDSL<'a, Var<RetT>>
where
    A: Into<ShaderDSL<'a, Var<InputT>>>,
    B: Into<ShaderDSL<'a, Var<InputT>>>,
    Var<InputT>: Into<FuncArg>,
{
    let a_dsl = Arc::new(a.into());
    let b_dsl = Arc::new(b.into());
    let op = Arc::new(op);
    _mdo_move! {
        [a_dsl, b_dsl, op]
        ident <- _new_ident();
        v1 <- (*a_dsl).clone();
        v2 <- (*b_dsl).clone();
        _call_func_rt((*op).clone(), vec![v1.into(), v2.into()], ident)
    }
}

pub fn dsl_builtin_3<'a, A, B, C, InputT: 'a, RetT>(
    op: FuncName,
    a: A,
    b: B,
    c: C,
) -> ShaderDSL<'a, Var<RetT>>
where
    A: Into<ShaderDSL<'a, Var<InputT>>>,
    B: Into<ShaderDSL<'a, Var<InputT>>>,
    C: Into<ShaderDSL<'a, Var<InputT>>>,
    Var<InputT>: Into<FuncArg>,
{
    let a_dsl = Arc::new(a.into());
    let b_dsl = Arc::new(b.into());
    let c_dsl = Arc::new(c.into());
    let op = Arc::new(op);
    _mdo_move! {
        [a_dsl, b_dsl, c_dsl, op]
        ident <- _new_ident();
        v1 <- (*a_dsl).clone();
        v2 <- (*b_dsl).clone();
        v3 <- (*c_dsl).clone();
        _call_func_rt((*op).clone(), vec![v1.into(), v2.into(), v3.into()], ident)
    }
}
pub fn normalize<'a, A, T>(a: A) -> ShaderDSL<'a, Var<T>>
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    dsl_builtin_1(FuncName::Normalize, a)
}

pub fn length<'a, A, T: 'a>(a: A) -> ShaderDSL<'a, Var<f32>>
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    dsl_builtin_1(FuncName::Length, a)
}

pub fn dot<'a, A, B, T: 'a>(a: A, b: B) -> ShaderDSL<'a, Var<f32>>
// Always returns f32
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    B: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    dsl_builtin_2(FuncName::Dot, a, b)
}

pub fn cross<'a, A, B>(a: A, b: B) -> ShaderDSL<'a, Var<Vec3<f32>>>
// Strictly Vec3
where
    A: Into<ShaderDSL<'a, Var<Vec3<f32>>>>,
    B: Into<ShaderDSL<'a, Var<Vec3<f32>>>>,
{
    dsl_builtin_2(FuncName::Cross, a, b)
}

pub fn pow<'a, A, B, T>(a: A, b: B) -> ShaderDSL<'a, Var<T>>
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    B: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    dsl_builtin_2(FuncName::Pow, a, b)
}

pub fn distance<'a, A, B, T: 'a>(a: A, b: B) -> ShaderDSL<'a, Var<f32>>
// Always returns f32
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    B: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    dsl_builtin_2(FuncName::Distance, a, b)
}

pub fn reflect<'a, A, B, T>(a: A, b: B) -> ShaderDSL<'a, Var<T>>
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    B: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    dsl_builtin_2(FuncName::Reflect, a, b)
}

pub fn mix<'a, A, B, C, T>(a: A, b: B, c: C) -> ShaderDSL<'a, Var<T>>
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    B: Into<ShaderDSL<'a, Var<T>>>,
    C: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    dsl_builtin_3(FuncName::Mix, a, b, c)
}

pub fn clamp<'a, A, B, C, T>(a: A, b: B, c: C) -> ShaderDSL<'a, Var<T>>
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    B: Into<ShaderDSL<'a, Var<T>>>,
    C: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    dsl_builtin_3(FuncName::Clamp, a, b, c)
}
macro_rules! impl_shader_lift {
    (
        $trait_name:ident,
        $( ($T:ident, $M:ident, $arg:ident, $arg_dsl:ident, $var:ident) ),+
    ) => {
        pub trait $trait_name<'a, $($T : 'a),*, Ret> {
            fn in_context<$($M),*>(&self, $($arg: $M),*) -> ShaderDSL<'a, Var<Ret>>
            where
                $($M: Into<ShaderDSL<'a, Var<$T>>>),*;
        }

        impl<'a, $($T : 'a),*, Ret : 'a, F> $trait_name<'a, $($T),*, Ret> for F
        where
            F: Fn($(Var<$T>),*) -> ShaderDSL<'a, Var<Ret>> + 'a + Clone,
            $($T: Clone + 'a),*
        {
            fn in_context<$($M),*>(&self, $($arg: $M),*) -> ShaderDSL<'a, Var<Ret>>
            where
                $($M: Into<ShaderDSL<'a, Var<$T>>>),*
            {
                $( let $arg_dsl = Arc::new($arg.into()); )*
                let f = self.clone();
                _mdo_move! {
                    [f, $($arg_dsl),*]
                    $( $var <- (*$arg_dsl).clone(); )*
                    f($($var),*)
                }
            }
        }
    };
}
impl_shader_lift!(ShaderLift1, (A, MA, ma, ma_dsl, a));

impl_shader_lift!(ShaderLift2, (A, MA, ma, ma_dsl, a), (B, MB, mb, mb_dsl, b));
impl_shader_lift!(
    ShaderLift3,
    (A, MA, ma, ma_dsl, a),
    (B, MB, mb, mb_dsl, b),
    (C, MC, mc, mc_dsl, c)
);
impl_shader_lift!(
    ShaderLift4,
    (A, MA, ma, ma_dsl, a),
    (B, MB, mb, mb_dsl, b),
    (C, MC, mc, mc_dsl, c),
    (D, MD, md, md_dsl, d)
);
