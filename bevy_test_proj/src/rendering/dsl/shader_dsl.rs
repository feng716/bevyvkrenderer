use std::{marker::PhantomData, sync::Arc};

use crate::rendering::dsl::builtin_func::{ForDSL, ForM, ShaderCmp, ShaderLift2, dot, length, set};
use crate::rendering::dsl::cast::ShaderCast;
use crate::{make_float4, mdo};

use super::monad::{lift_f, CloneWrapped, Free, OwnedApplicative, OwnedFunctor, OwnedMonad};
use super::vec_op::{make_float4_impl, Vec2, Vec3, Vec4};

pub struct Var<T> {
    pub(crate) ident: i32,
    pub(crate) _marker: PhantomData<T>,
}
impl<T> Clone for Var<T> {
    fn clone(&self) -> Self {
        Self {
            ident: self.ident.clone(),
            _marker: PhantomData,
        }
    }
}
impl<T> Copy for Var<T> {}
impl<T> ToString for Var<T> {
    fn to_string(&self) -> String {
        format!("v{}", self.ident)
    }
}

pub(super) enum ShaderDSLF<'a, T> {
    NewIdent(Arc<dyn Fn(i32) -> T + 'a>),
    BeginScope(T),
    EndScope(T),
    If(Var<bool>, T),
    Else(T),
    Loop(T),
    Continuing(T),
    Set(String, T),
    Call(Option<String>, FuncName, Vec<FuncArg>, T),
    Return(String, T)
}
#[derive(Clone)]
pub(super) enum FuncName {
    MakeFloat4,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Neg,
    Not,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    Dot,
    Cross,
    Pow,
    Normalize,
    Length,
    Distance,
    Mix,
    Clamp,
    Reflect,
    Refract,
    MakeFloat3,
    MakeFloat2,
    Break,
    Continue,
    Cast(&'static str),
}
#[derive(Clone, shader_macros::DisplayInner)]
pub(super) enum FuncArg {
    F32(Var<f32>),
    F32_2(Var<Vec2<f32>>),
    F32_3(Var<Vec3<f32>>),
    F32_4(Var<Vec4<f32>>),
    Bool(Var<bool>),
    I32(Var<i32>),
    U32(Var<u32>),
}
impl From<Var<f32>> for FuncArg {
    fn from(v: Var<f32>) -> Self {
        FuncArg::F32(v)
    }
}
impl From<Var<Vec2<f32>>> for FuncArg {
    fn from(v: Var<Vec2<f32>>) -> Self {
        FuncArg::F32_2(v)
    }
}
impl From<Var<Vec3<f32>>> for FuncArg {
    fn from(v: Var<Vec3<f32>>) -> Self {
        FuncArg::F32_3(v)
    }
}
impl From<Var<Vec4<f32>>> for FuncArg {
    fn from(v: Var<Vec4<f32>>) -> Self {
        FuncArg::F32_4(v)
    }
}
impl From<Var<bool>> for FuncArg {
    fn from(v: Var<bool>) -> Self {
        FuncArg::Bool(v)
    }
}
impl From<Var<i32>> for FuncArg {
    fn from(v: Var<i32>) -> Self {
        FuncArg::I32(v)
    }
}
impl From<Var<u32>> for FuncArg {
    fn from(v: Var<u32>) -> Self {
        FuncArg::U32(v)
    }
}
impl<'a, T> Clone for ShaderDSLF<'a, T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::NewIdent(arg0) => Self::NewIdent(arg0.clone()),
            Self::BeginScope(arg0) => Self::BeginScope(arg0.clone()),
            Self::EndScope(arg0) => Self::EndScope(arg0.clone()),
            Self::If(arg0, arg1) => Self::If(arg0.clone(), arg1.clone()),
            Self::Else(arg0) => Self::Else(arg0.clone()),
            Self::Loop(arg0) => Self::Loop(arg0.clone()),
            Self::Continuing(arg0) => Self::Continuing(arg0.clone()),
            Self::Set(arg0, arg1) => Self::Set(arg0.clone(), arg1.clone()),
            Self::Call(arg0, arg1, arg2, arg3) => Self::Call(arg0.clone(), arg1.clone(), arg2.clone(), arg3.clone()),
            Self::Return(arg0, arg1) => Self::Return(arg0.clone(), arg1.clone()),
        }
    }
}
impl<'b, T> OwnedFunctor<'b> for ShaderDSLF<'b, T>
where
    T: 'b,
{
    type OwnedUnwrapped = T;

    type OwnedWrapped<'c, T1>
        = ShaderDSLF<'c, T1>
    where
        'b: 'c,
        T1: 'b;

    fn fmap<'c, B: 'b, F>(self, f: F) -> Self::OwnedWrapped<'c, B>
    where
        'b: 'c,
        F: 'c + Fn(Self::OwnedUnwrapped) -> B,
    {
        match self {
            ShaderDSLF::NewIdent(n_f) => ShaderDSLF::NewIdent(Arc::new(move |x| f(n_f(x)))),
            ShaderDSLF::BeginScope(t) => ShaderDSLF::BeginScope(f(t)),
            ShaderDSLF::EndScope(t) => ShaderDSLF::EndScope(f(t)),
            ShaderDSLF::If(var, t) => ShaderDSLF::If(var, f(t)),
            ShaderDSLF::Else(t) => ShaderDSLF::Else(f(t)),
            ShaderDSLF::Loop(t) => ShaderDSLF::Loop(f(t)),
            ShaderDSLF::Set(var, t) => ShaderDSLF::Set(var, f(t)),
            ShaderDSLF::Call(rt, func_name, func_args, t) => {
                ShaderDSLF::Call(rt, func_name, func_args, f(t))
            }
            ShaderDSLF::Continuing(t) => ShaderDSLF::Continuing(f(t)),
            ShaderDSLF::Return(s, t) => ShaderDSLF::Return(s, f(t)),
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
impl<'b, T> CloneWrapped<'b> for ShaderDSLF<'b, T>
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

pub type ShaderDSL<'a, T> = Free<'a, ShaderDSLF<'a, T>, T>;
pub(super) fn _new_ident<'a, T>() -> ShaderDSL<'a, Var<T>> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::NewIdent(Arc::new(|x| Var {
        ident: x,
        _marker: PhantomData,
    })))
}
pub(super) fn _begin_scope<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::BeginScope(()))
}
pub(super) fn _end_scope<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::EndScope(()))
}
pub(super) fn _if_statement<'a>(cond: Var<bool>) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::If(cond, ()))
}
pub(super) fn _else_statement<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Else(()))
}
pub(super) fn _continuing_statement<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Continuing(()))
}
pub(super) fn _call_func_rt<'a, T: Clone + ToString>(
    f: FuncName,
    args: Vec<FuncArg>,
    rt: T,
) -> ShaderDSL<'a, T> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Call(Some(rt.to_string()), f, args, rt))
}
pub(super) fn _call_func<'a>(f: FuncName, args: Vec<FuncArg>) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Call(None, f, args, ()))
}
pub(super) fn _set_statement_let<'a>(v: String, assigned: String) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Set(format!("let {} = {}", v, assigned), ()))
}
pub(super) fn _set_statement_reassign<'a>(v: String, assigned: String) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Set(format!("{} = {}", v, assigned), ()))
}
pub(super) fn _loop_statement<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Loop(()))
}
pub(super) fn _return_statement<'a, T : ToString>(a : T) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Return(a.to_string(), ()))
}
pub(super) fn _break_statement<'a>() -> ShaderDSL<'a, ()> {
    _call_func(FuncName::Break, vec![])
}

pub(super) fn _continue_statement<'a>() -> ShaderDSL<'a, ()> {
    _call_func(FuncName::Continue, vec![])
}
#[derive(Clone)]
struct IfBuilder<'a, T: Clone> {
    condition: ShaderDSL<'a, Var<bool>>,
    true_branch: ShaderDSL<'a, T>,
}
pub fn if_<'a, T: Clone, T1: Into<ShaderDSL<'a, Var<bool>>>>(
    cond: T1,
    true_statements: ShaderDSL<'a, T>,
) -> IfBuilder<'a, T> {
    IfBuilder {
        condition: cond.into(),
        true_branch: true_statements,
    }
}
#[macro_export]
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
        let s1 = Arc::new(self.true_branch);
        let s2 = Arc::new(false_statements);
        _mdo_move! {
            [s1, s2]
            v <- self.condition;
            _if_statement(v);
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
        T: Copy,
        F: 'a + Fn(T) -> ShaderDSL<'a, ()> + Clone,
    {
        let s = Arc::new(self.true_branch);
        _mdo_move! {
            [s, f]
            v <- self.condition;
            _if_statement(v);
            _begin_scope();
            x <- (*s).clone();
            _end_scope();
            f(x)
        }
    }
}



pub fn while_<'a, Cond, Body>(
    cond_fn: Cond,
    body: Body,
) -> ShaderDSL<'a, ()>
where
    Cond: 'a + Fn() -> ShaderDSL<'a, Var<bool>>,
    Body: 'a + Fn((fn() -> ShaderDSL<'a, ()>, fn() -> ShaderDSL<'a, ()>)) -> ShaderDSL<'a, ()>
{
    let cond_fn_arc = Arc::new(cond_fn);
    let body_arc = Arc::new(body((_break_statement, _continue_statement)));

    _mdo_move! {
        [cond_fn_arc, body_arc]
        
        initial_cond <- (*cond_fn_arc)();
        
        _loop_statement();
        _begin_scope();
        
        v <- !initial_cond.in_context();
        _if_statement(v);
        _begin_scope();
        _break_statement();
        _end_scope();
        
        _tmp <- (*body_arc).clone();
        
        _continuing_statement();
        _begin_scope();
        
        next_cond <- (*cond_fn_arc)();
        set(initial_cond, next_cond);
        
        _end_scope(); 
        _end_scope()
    }
}

fn _build_shader<'a, T>(v: ShaderDSL<'a, T>, str: String, ident: i32) -> String {
    match v {
        Free::Pure(a) => str,
        Free::Free(step) => match *step {
            ShaderDSLF::NewIdent(next_f) => _build_shader(next_f(ident), str, ident + 1),
            ShaderDSLF::BeginScope(next_prog) => _build_shader(next_prog, str + "\n{", ident),
            ShaderDSLF::EndScope(next_prog) => _build_shader(next_prog, str + "\n}", ident),
            ShaderDSLF::If(var, next_prog) => _build_shader(
                next_prog,
                str + format!("\nif(v{})", var.ident).as_str(),
                ident,
            ),
            ShaderDSLF::Else(next_prog) => _build_shader(next_prog, str + "\nelse", ident),
            ShaderDSLF::Loop(t) => {
                _build_shader(t, str + "\nloop", ident)
            }
            ShaderDSLF::Set(s, t) => _build_shader(t, str + format!("\n{};", s).as_str(), ident),
            ShaderDSLF::Call(rt, func_name, func_args, t) => {
                let v: Vec<_> = func_args.iter().map(|v| v.to_string()).collect();
                // {}({})
                let call_fn = |fn_name| match fn_name {
                    FuncName::MakeFloat4 => format!("vec4f({})", v.join(",")),
                    FuncName::MakeFloat3 => format!("vec3f({})", v.join(",")),
                    FuncName::MakeFloat2 => format!("vec2f({})", v.join(",")),
                    FuncName::Add => format!("{} + {}", v[0], v[1]),
                    FuncName::Sub => format!("{} - {}", v[0], v[1]),
                    FuncName::Mul => format!("{} * {}", v[0], v[1]),
                    FuncName::Div => format!("{} / {}", v[0], v[1]),
                    FuncName::Rem => format!("{} % {}", v[0], v[1]),
                    FuncName::BitAnd => format!("{} & {}", v[0], v[1]),
                    FuncName::BitOr => format!("{} | {}", v[0], v[1]),
                    FuncName::BitXor => format!("{} ^ {}", v[0], v[1]),
                    FuncName::Shl => format!("{} << {}", v[0], v[1]),
                    FuncName::Shr => format!("{} >> {}", v[0], v[1]),
                    FuncName::Neg => format!("-{}", v[0]),
                    FuncName::Not => format!("!{}", v[0]),
                    FuncName::Eq => format!("{} == {}", v[0], v[1]),
                    FuncName::Neq => format!("{} != {}", v[0], v[1]),
                    FuncName::Lt => format!("{} < {}", v[0], v[1]),
                    FuncName::Lte => format!("{} <= {}", v[0], v[1]),
                    FuncName::Gt => format!("{} > {}", v[0], v[1]),
                    FuncName::Gte => format!("{} >= {}", v[0], v[1]),
                    FuncName::Normalize => format!("normalize({})", v[0]),
                    FuncName::Length => format!("length({})", v[0]),
                    FuncName::Dot => format!("dot({}, {})", v[0], v[1]),
                    FuncName::Cross => format!("cross({}, {})", v[0], v[1]),
                    FuncName::Pow => format!("pow({}, {})", v[0], v[1]),
                    FuncName::Distance => format!("distance({}, {})", v[0], v[1]),
                    FuncName::Reflect => format!("reflect({}, {})", v[0], v[1]),
                    FuncName::Mix => format!("mix({}, {}, {})", v[0], v[1], v[2]),
                    FuncName::Clamp => format!("clamp({}, {}, {})", v[0], v[1], v[2]),
                    FuncName::Refract => format!("refract({}, {}, {})", v[0], v[1], v[2]),
                    FuncName::Break => "break".to_string(),
                    FuncName::Continue => "continue".to_string(),
                    FuncName::Cast(t) => format!("{}({})", t, v[0]),
                };
                _build_shader(
                    t,
                    format!(
                        "{}\n{}{};",
                        str,
                        rt.map_or(String::from(""), |v| format!("let {} = ", v)),
                        call_fn(func_name)
                    ),
                    ident,
                )
            }
            ShaderDSLF::Continuing(t) => _build_shader(t, str + "\ncontinuing", ident),
            ShaderDSLF::Return(s, t) => todo!(),
        },
    }
}
impl<'a, T> From<Var<T>> for ShaderDSL<'a, Var<T>> {
    fn from(val: Var<T>) -> Self {
        Free::Pure(val)
    }
}

impl<'a> From<i32> for ShaderDSL<'a, Var<i32>> {
    fn from(value: i32) -> Self {
        mdo! {
            val <- _new_ident();
            _set_statement_let(val.to_string(), value.to_string());
            Free::Pure(val)
        }
    }
}
impl<'a> From<f32> for ShaderDSL<'a, Var<f32>> {
    fn from(value: f32) -> Self {
        mdo! {
            val <- _new_ident();
            _set_statement_let(val.to_string(), format!("{}f", value.to_string()));
            Free::Pure(val)
        }
    }
}
impl<'a> From<bool> for ShaderDSL<'a, Var<bool>> {
    fn from(value: bool) -> Self {
        mdo! {
            val <- _new_ident();
            _set_statement_let(val.to_string(), value.to_string());
            Free::Pure(val)
        }
    }
}

pub(super) trait IntoShaderVar<'a> {
    type InnerT;
    fn in_context(self) -> ShaderDSL<'a, Var<Self::InnerT>>;
}

impl<'a> IntoShaderVar<'a> for f32 {
    type InnerT = f32;
    fn in_context(self) -> ShaderDSL<'a, Var<f32>> {
        self.into()
    }
}

impl<'a, T> IntoShaderVar<'a> for ShaderDSL<'a, Var<T>> {
    type InnerT = T;
    fn in_context(self) -> ShaderDSL<'a, Var<T>> {
        self
    }
}

impl<'a, T: 'a> IntoShaderVar<'a> for Var<T> {
    type InnerT = T;
    fn in_context(self) -> ShaderDSL<'a, Var<T>> {
        self.into()
    }
}

#[test]
fn test_dsl() {
    let c1 = |a: Var<f32>, b: Var<f32>| {
        mdo! {
            a.in_context().lt(b)
        }
    };
    let program: Free<'_, ShaderDSLF<'_, ()>, ()> = mdo! {
        val2 <- make_float4!(1.);
        val3 <- make_float4!(val2.x());
        _a <- val2 + val3.in_context();
        if_(c1.in_context(length(val2), length(val3)), mdo!{
            val5 <- make_float4!(val2.xy(), _a.xy());
        });
        val6 <- dot(val2, val3);
        set(val2.x(), val2.x());
        (1..=3).forM_(|i|mdo!{
            val <- make_float4!(i as f32);
        });
        while_(move||val2.x().lt(val2.y()), |(break_, _)|mdo!{
            val <- make_float4!(1.);
            break_();
        });
        (1..2).for_(|i, _| mdo!{
            val <- make_float4!(i.cast::<f32>());
        });
        
        val5 <- length(val2);
    };
    // println!("{}", _build_shader(program, String::from(""), 0));
}
