use std::{
    marker::PhantomData,
    ops::{Add, Div, Mul, Sub},
    sync::Arc,
};

use crate::rendering::{
    self,
    monad::{lift_f, CloneWrapped, Free, OwnedApplicative, OwnedFunctor, OwnedMonad},
};

pub struct Var<T> {
    ident: i32,
    _marker: PhantomData<T>,
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

// shuffle .xyz
// if else -> if_else(condition, Statements, Statements)
// iteration to for
// while -> while_(\break -> (condition, {})) -> While(Condition, Statements)
// call member function -> Access(Var, Function)
// call -> Call
// set -> Set

/// TODO)): turn this into compile-time
#[derive(Clone)]
enum VarAccessExpr<T> {
    // T1 and T2 should be ShaderDSL<'a, i32>
    // potentially .xx() cannot be LVar
    VectorAccessSwizzling(Box<VarAccessExpr<T>>, u32), // ternary or quaternary
    MatrixAccessSwizzling(Box<VarAccessExpr<T>>, u32, u32), // ternary or quaternary
    Var(T),
    Array1DAccess(Box<VarAccessExpr<T>>, Var<i32>),
    Array2DAccess(Box<VarAccessExpr<T>>, Var<i32>, Var<i32>),
}
// TODO)): refactor the generic
fn access_to_string<T>(k: &VarAccessExpr<Var<T>>) -> String {
    match k {
        VarAccessExpr::VectorAccessSwizzling(var_access_expr, swizzle) => format!(
            "{}.{}",
            var_access_expr.to_string(),
            decode_swizzle(*swizzle)
        ),
        VarAccessExpr::MatrixAccessSwizzling(var_access_expr, _, _) => todo!(),
        VarAccessExpr::Var(v) => v.to_string(),
        VarAccessExpr::Array1DAccess(var_access_expr, i) => {
            format!("{}[{}]", var_access_expr.to_string(), i.to_string())
        }
        VarAccessExpr::Array2DAccess(var_access_expr, i, j) => format!(
            "{}[{}][{}]",
            var_access_expr.to_string(),
            i.to_string(),
            j.to_string()
        ),
    }
}
impl<T> ToString for VarAccessExpr<Var<T>> {
    fn to_string(&self) -> String {
        access_to_string(self)
    }
}
#[derive(Clone)]
struct TypedAccessExpr<T, CurrentT> {
    v: VarAccessExpr<T>,
    _marker: PhantomData<CurrentT>,
}
impl<T, CurrentT> ToString for TypedAccessExpr<Var<T>, CurrentT> {
    fn to_string(&self) -> String {
        access_to_string(&self.v)
    }
}
struct VarAccess<T> {
    v: Var<T>,
    v_access: VarAccessExpr<Var<T>>,
}
impl<T> Clone for VarAccess<T> {
    fn clone(&self) -> Self {
        Self {
            v: self.v.clone(),
            v_access: self.v_access.clone(),
        }
    }
}
impl<T> ToString for VarAccess<T> {
    fn to_string(&self) -> String {
        access_to_string(&self.v_access)
    }
}

// Receiving RValue : Into<ShaderDSL<'a, T>>, impl this for Var<T>, VarAccessExpr,
// Receiving LValue : LVarExpr
// Raw Var<T> is LValue
enum ShaderDSLF<'a, T> {
    NewIdent(Arc<dyn Fn(i32) -> T + 'a>),
    BeginScope(T),
    EndScope(T),
    If(Var<bool>, T),
    Else(T),
    While(Var<bool>, T),
    Set(String, T),
    Call(Option<String>, FuncName, Vec<FuncArg>, T),
}
#[derive(Clone)]
enum FuncName {
    MakeFloat4,
    Add,
    Sub,
    Mul,
    Div,
}
#[derive(Clone, shader_macros::DisplayInner)]
enum FuncArg {
    F32(Var<f32>),
    F32_2(Var<Vec2<f32>>),
    F32_3(Var<Vec3<f32>>),
    F32_4(Var<Vec4<f32>>),
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
            Self::While(arg0, arg1) => Self::While(arg0.clone(), arg1.clone()),
            Self::Set(arg0, arg1) => Self::Set(arg0.clone(), arg1.clone()),
            Self::Call(arg0, arg1, arg2, arg3) => {
                Self::Call(arg0.clone(), arg1.clone(), arg2.clone(), arg3.clone())
            }
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
            ShaderDSLF::While(var, t) => ShaderDSLF::While(var, f(t)),
            ShaderDSLF::Set(var, t) => ShaderDSLF::Set(var, f(t)),
            ShaderDSLF::Call(rt, func_name, func_args, t) => {
                ShaderDSLF::Call(rt, func_name, func_args, f(t))
            }
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

type ShaderDSL<'a, T> = Free<'a, ShaderDSLF<'a, T>, T>;
fn _new_ident<'a, T>() -> ShaderDSL<'a, Var<T>> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::NewIdent(Arc::new(|x| Var {
        ident: x,
        _marker: PhantomData,
    })))
}
fn _begin_scope<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::BeginScope(()))
}
fn _end_scope<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::EndScope(()))
}
fn _if_statement<'a>(cond: Var<bool>) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::If(cond, ()))
}
fn _else_statement<'a>() -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Else(()))
}
fn _call_func_rt<'a, T: Clone + ToString>(
    f: FuncName,
    args: Vec<FuncArg>,
    rt: T,
) -> ShaderDSL<'a, T> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Call(Some(rt.to_string()), f, args, rt))
}
fn _call_func<'a>(f: FuncName, args: Vec<FuncArg>) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Call(None, f, args, ()))
}
fn _set_statement<'a>(v: String, assigned: String) -> ShaderDSL<'a, ()> {
    lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Set(format!("{} = {}", v, assigned), ()))
}

#[derive(Clone)]
struct IfBuilder<'a, T: Clone> {
    condition: Var<bool>,
    true_branch: ShaderDSL<'a, T>,
}
pub fn if_<'a, T: Clone, T1: Into<Var<bool>>>(
    cond: T1,
    true_statements: ShaderDSL<'a, T>,
) -> IfBuilder<'a, T> {
    IfBuilder {
        condition: cond.into(),
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
        let s1 = Arc::new(self.true_branch);
        let s2 = Arc::new(false_statements);
        _mdo_move! {
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
        _mdo_move! {
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

const fn encode_swizzle(s: &str) -> u32 {
    let bytes = s.as_bytes();
    let mut mask: u32 = 0;
    let mut i = 0;

    // 1. Store the length in the highest 8 bits
    mask |= (bytes.len() as u32) << 24;

    // 2. Iterate through characters and pack them into 4-bit slots
    while i < bytes.len() {
        let val = match bytes[i] {
            b'x' | b'r' => 0,
            b'y' | b'g' => 1,
            b'z' | b'b' => 2,
            b'w' | b'a' => 3,
            _ => panic!("Invalid swizzle character"),
        };

        // Shift by i * 4 bits (Component 0 at bits 0-3, Component 1 at bits 4-7, etc.)
        mask |= (val as u32) << (i * 4);
        i += 1;
    }

    mask
}
fn decode_swizzle(mask: u32) -> String {
    let len = (mask >> 24) & 0xFF;
    let mut result = String::with_capacity(len as usize);

    for i in 0..len {
        let component_index = (mask >> (i * 4)) & 0xF;
        let char = match component_index {
            0 => 'x',
            1 => 'y',
            2 => 'z',
            3 => 'w',
            _ => unreachable!(),
        };
        result.push(char);
    }

    result
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
            ShaderDSLF::While(var, t) => {
                _build_shader(t, str + format!("\nwhile(v{})", var.ident).as_str(), ident)
            }
            ShaderDSLF::Set(s, t) => _build_shader(t, str + format!("\n{};", s).as_str(), ident),
            ShaderDSLF::Call(rt, func_name, func_args, t) => {
                let v: Vec<_> = func_args.iter().map(|v| v.to_string()).collect();
                // {}({})
                let call_fn = |fn_name| match fn_name {
                    FuncName::MakeFloat4 => format!("vec4f({})", v.join(",")),
                    FuncName::Add => format!("{} + {}", v[0], v[1]),
                    FuncName::Sub => format!("{} - {}", v[0], v[1]),
                    FuncName::Mul => format!("{} * {}", v[0], v[1]),
                    FuncName::Div => format!("{} / {}", v[0], v[1]),
                };
                _build_shader(
                    t,
                    format!(
                        "{}\n{}{};",
                        str,
                        rt.map_or(String::from(""), |v| format!("{} = ", v)),
                        call_fn(func_name)
                    ),
                    ident,
                )
            }
        },
    }
}
impl<'a, T> From<Var<T>> for ShaderDSL<'a, Var<T>> {
    fn from(val: Var<T>) -> Self {
        Free::Pure(val)
    }
}
impl<'a, T: 'a> From<VarAccess<T>> for ShaderDSL<'a, Var<T>> {
    fn from(val: VarAccess<T>) -> Self {
        Free::Pure(val.v)
    }
}
impl<'a, T: 'a, CurrentT: Clone> From<TypedAccessExpr<Var<T>, CurrentT>>
    for ShaderDSL<'a, Var<CurrentT>>
{
    fn from(val: TypedAccessExpr<Var<T>, CurrentT>) -> Self {
        mdo! {
            new_ident <- _new_ident();
            _set_statement(new_ident.to_string(), val.to_string());
            Free::Pure(new_ident)
        }
    }
}
impl<'a> From<i32> for ShaderDSL<'a, Var<i32>> {
    fn from(value: i32) -> Self {
        mdo! {
            val <- _new_ident();
            _set_statement(val.to_string(), value.to_string());
            rendering::monad::Free::Pure(val)
        }
    }
}
impl<'a> From<f32> for ShaderDSL<'a, Var<f32>> {
    fn from(value: f32) -> Self {
        mdo! {
            val <- _new_ident();
            _set_statement(val.to_string(), value.to_string());
            rendering::monad::Free::Pure(val)
        }
    }
}
impl<'a> From<bool> for ShaderDSL<'a, Var<bool>> {
    fn from(value: bool) -> Self {
        mdo! {
            val <- _new_ident();
            _set_statement(val.to_string(), value.to_string());
            rendering::monad::Free::Pure(val)
        }
    }
}
#[derive(Clone, Copy)]
struct Vec4<T>(PhantomData<T>);
#[derive(Clone, Copy)]
struct Vec3<T>(PhantomData<T>);
#[derive(Clone, Copy)]
struct Vec2<T>(PhantomData<T>);
#[derive(Clone, Copy)]
struct Array1D<T>(PhantomData<T>);
#[derive(Clone, Copy)]
struct Array2D<T>(PhantomData<T>);

macro_rules! typed_expr_swizzle {
    ($name:ident, $ret:ident) => {
        pub fn $name(self) -> TypedAccessExpr<BaseVar, $ret<T>> {
            const MASK: u32 = encode_swizzle(stringify!($name));
            TypedAccessExpr {
                v: VarAccessExpr::VectorAccessSwizzling(Box::new(self.v), MASK),
                _marker: PhantomData,
            }
        }
    };
}
macro_rules! typed_expr_swizzle_scalar {
    ($name:ident) => {
        pub fn $name(self) -> TypedAccessExpr<BaseVar, T> {
            const MASK: u32 = encode_swizzle(stringify!($name));
            TypedAccessExpr {
                v: VarAccessExpr::VectorAccessSwizzling(Box::new(self.v), MASK),
                _marker: PhantomData,
            }
        }
    };
}
macro_rules! var_access_swizzle {
    ($name:ident, $source:ident, $ret:ident) => {
        pub fn $name(self) -> TypedAccessExpr<Var<$source<T>>, $ret<T>> {
            const MASK: u32 = encode_swizzle(stringify!($name));
            TypedAccessExpr {
                v: VarAccessExpr::VectorAccessSwizzling(Box::new(VarAccessExpr::Var(self)), MASK),
                _marker: PhantomData,
            }
        }
    };
}

macro_rules! var_access_swizzle_scalar {
    ($name:ident, $source:ident) => {
        pub fn $name(self) -> TypedAccessExpr<Var<$source<T>>, T> {
            const MASK: u32 = encode_swizzle(stringify!($name));
            TypedAccessExpr {
                v: VarAccessExpr::VectorAccessSwizzling(Box::new(VarAccessExpr::Var(self)), MASK),
                _marker: PhantomData,
            }
        }
    };
}
impl<BaseVar, T> TypedAccessExpr<BaseVar, Vec4<T>> {
    typed_expr_swizzle_scalar!(x);
    typed_expr_swizzle_scalar!(y);
    typed_expr_swizzle_scalar!(z);
    typed_expr_swizzle_scalar!(w);

    typed_expr_swizzle!(xy, Vec2);
    typed_expr_swizzle!(xz, Vec2);
    typed_expr_swizzle!(xw, Vec2);
    typed_expr_swizzle!(yx, Vec2);
    typed_expr_swizzle!(yz, Vec2);
    typed_expr_swizzle!(yw, Vec2);
    typed_expr_swizzle!(zx, Vec2);
    typed_expr_swizzle!(zy, Vec2);
    typed_expr_swizzle!(zw, Vec2);
    typed_expr_swizzle!(wx, Vec2);
    typed_expr_swizzle!(wy, Vec2);
    typed_expr_swizzle!(wz, Vec2);

    typed_expr_swizzle!(xyz, Vec3);
    typed_expr_swizzle!(xyw, Vec3);
    typed_expr_swizzle!(xzy, Vec3);
    typed_expr_swizzle!(xzw, Vec3);
    typed_expr_swizzle!(xwy, Vec3);
    typed_expr_swizzle!(xwz, Vec3);
    typed_expr_swizzle!(yxz, Vec3);
    typed_expr_swizzle!(yxw, Vec3);
    typed_expr_swizzle!(yzx, Vec3);
    typed_expr_swizzle!(yzw, Vec3);
    typed_expr_swizzle!(ywx, Vec3);
    typed_expr_swizzle!(ywz, Vec3);
    typed_expr_swizzle!(zxy, Vec3);
    typed_expr_swizzle!(zxw, Vec3);
    typed_expr_swizzle!(zyx, Vec3);
    typed_expr_swizzle!(zyw, Vec3);
    typed_expr_swizzle!(zwx, Vec3);
    typed_expr_swizzle!(zwy, Vec3);
    typed_expr_swizzle!(wxy, Vec3);
    typed_expr_swizzle!(wxz, Vec3);
    typed_expr_swizzle!(wyx, Vec3);
    typed_expr_swizzle!(wyz, Vec3);
    typed_expr_swizzle!(wzx, Vec3);
    typed_expr_swizzle!(wzy, Vec3);

    typed_expr_swizzle!(xyzw, Vec4);
    typed_expr_swizzle!(xywz, Vec4);
    typed_expr_swizzle!(xzyw, Vec4);
    typed_expr_swizzle!(xzwy, Vec4);
    typed_expr_swizzle!(xwyz, Vec4);
    typed_expr_swizzle!(xwzy, Vec4);
    typed_expr_swizzle!(yxzw, Vec4);
    typed_expr_swizzle!(yxwz, Vec4);
    typed_expr_swizzle!(yzxw, Vec4);
    typed_expr_swizzle!(yzwx, Vec4);
    typed_expr_swizzle!(ywxz, Vec4);
    typed_expr_swizzle!(ywzx, Vec4);
    typed_expr_swizzle!(zxyw, Vec4);
    typed_expr_swizzle!(zxwy, Vec4);
    typed_expr_swizzle!(zyxw, Vec4);
    typed_expr_swizzle!(zywx, Vec4);
    typed_expr_swizzle!(zwxy, Vec4);
    typed_expr_swizzle!(zwyx, Vec4);
    typed_expr_swizzle!(wxyz, Vec4);
    typed_expr_swizzle!(wxzy, Vec4);
    typed_expr_swizzle!(wyxz, Vec4);
    typed_expr_swizzle!(wyzx, Vec4);
    typed_expr_swizzle!(wzxy, Vec4);
    typed_expr_swizzle!(wzyx, Vec4);
}
impl<BaseVar, T> TypedAccessExpr<BaseVar, Vec3<T>> {
    typed_expr_swizzle_scalar!(x);
    typed_expr_swizzle_scalar!(y);
    typed_expr_swizzle_scalar!(z);

    typed_expr_swizzle!(xy, Vec2);
    typed_expr_swizzle!(xz, Vec2);
    typed_expr_swizzle!(yx, Vec2);
    typed_expr_swizzle!(yz, Vec2);
    typed_expr_swizzle!(zx, Vec2);
    typed_expr_swizzle!(zy, Vec2);

    typed_expr_swizzle!(xyz, Vec3);
    typed_expr_swizzle!(xzy, Vec3);
    typed_expr_swizzle!(yxz, Vec3);
    typed_expr_swizzle!(yzx, Vec3);
    typed_expr_swizzle!(zxy, Vec3);
    typed_expr_swizzle!(zyx, Vec3);
}
impl<BaseVar, T> TypedAccessExpr<BaseVar, Vec2<T>> {
    typed_expr_swizzle_scalar!(x);
    typed_expr_swizzle_scalar!(y);

    typed_expr_swizzle!(xy, Vec2);
    typed_expr_swizzle!(yx, Vec2);
}
impl<T> Var<Vec4<T>> {
    var_access_swizzle_scalar!(x, Vec4);
    var_access_swizzle_scalar!(y, Vec4);
    var_access_swizzle_scalar!(z, Vec4);
    var_access_swizzle_scalar!(w, Vec4);

    var_access_swizzle!(xy, Vec4, Vec2);
    var_access_swizzle!(xz, Vec4, Vec2);
    var_access_swizzle!(xw, Vec4, Vec2);
    var_access_swizzle!(yx, Vec4, Vec2);
    var_access_swizzle!(yz, Vec4, Vec2);
    var_access_swizzle!(yw, Vec4, Vec2);
    var_access_swizzle!(zx, Vec4, Vec2);
    var_access_swizzle!(zy, Vec4, Vec2);
    var_access_swizzle!(zw, Vec4, Vec2);
    var_access_swizzle!(wx, Vec4, Vec2);
    var_access_swizzle!(wy, Vec4, Vec2);
    var_access_swizzle!(wz, Vec4, Vec2);

    var_access_swizzle!(xyz, Vec4, Vec3);
    var_access_swizzle!(xyw, Vec4, Vec3);
    var_access_swizzle!(xzy, Vec4, Vec3);
    var_access_swizzle!(xzw, Vec4, Vec3);
    var_access_swizzle!(xwy, Vec4, Vec3);
    var_access_swizzle!(xwz, Vec4, Vec3);
    var_access_swizzle!(yxz, Vec4, Vec3);
    var_access_swizzle!(yxw, Vec4, Vec3);
    var_access_swizzle!(yzx, Vec4, Vec3);
    var_access_swizzle!(yzw, Vec4, Vec3);
    var_access_swizzle!(ywx, Vec4, Vec3);
    var_access_swizzle!(ywz, Vec4, Vec3);
    var_access_swizzle!(zxy, Vec4, Vec3);
    var_access_swizzle!(zxw, Vec4, Vec3);
    var_access_swizzle!(zyx, Vec4, Vec3);
    var_access_swizzle!(zyw, Vec4, Vec3);
    var_access_swizzle!(zwx, Vec4, Vec3);
    var_access_swizzle!(zwy, Vec4, Vec3);
    var_access_swizzle!(wxy, Vec4, Vec3);
    var_access_swizzle!(wxz, Vec4, Vec3);
    var_access_swizzle!(wyx, Vec4, Vec3);
    var_access_swizzle!(wyz, Vec4, Vec3);
    var_access_swizzle!(wzx, Vec4, Vec3);
    var_access_swizzle!(wzy, Vec4, Vec3);

    var_access_swizzle!(xyzw, Vec4, Vec4);
    var_access_swizzle!(xywz, Vec4, Vec4);
    var_access_swizzle!(xzyw, Vec4, Vec4);
    var_access_swizzle!(xzwy, Vec4, Vec4);
    var_access_swizzle!(xwyz, Vec4, Vec4);
    var_access_swizzle!(xwzy, Vec4, Vec4);
    var_access_swizzle!(yxzw, Vec4, Vec4);
    var_access_swizzle!(yxwz, Vec4, Vec4);
    var_access_swizzle!(yzxw, Vec4, Vec4);
    var_access_swizzle!(yzwx, Vec4, Vec4);
    var_access_swizzle!(ywxz, Vec4, Vec4);
    var_access_swizzle!(ywzx, Vec4, Vec4);
    var_access_swizzle!(zxyw, Vec4, Vec4);
    var_access_swizzle!(zxwy, Vec4, Vec4);
    var_access_swizzle!(zyxw, Vec4, Vec4);
    var_access_swizzle!(zywx, Vec4, Vec4);
    var_access_swizzle!(zwxy, Vec4, Vec4);
    var_access_swizzle!(zwyx, Vec4, Vec4);
    var_access_swizzle!(wxyz, Vec4, Vec4);
    var_access_swizzle!(wxzy, Vec4, Vec4);
    var_access_swizzle!(wyxz, Vec4, Vec4);
    var_access_swizzle!(wyzx, Vec4, Vec4);
    var_access_swizzle!(wzxy, Vec4, Vec4);
    var_access_swizzle!(wzyx, Vec4, Vec4);
}

// ==========================================
// VEC3 INITIAL ACCESS
// ==========================================
impl<T> Var<Vec3<T>> {
    var_access_swizzle_scalar!(x, Vec3);
    var_access_swizzle_scalar!(y, Vec3);
    var_access_swizzle_scalar!(z, Vec3);

    var_access_swizzle!(xy, Vec3, Vec2);
    var_access_swizzle!(xz, Vec3, Vec2);
    var_access_swizzle!(yx, Vec3, Vec2);
    var_access_swizzle!(yz, Vec3, Vec2);
    var_access_swizzle!(zx, Vec3, Vec2);
    var_access_swizzle!(zy, Vec3, Vec2);

    var_access_swizzle!(xyz, Vec3, Vec3);
    var_access_swizzle!(xzy, Vec3, Vec3);
    var_access_swizzle!(yxz, Vec3, Vec3);
    var_access_swizzle!(yzx, Vec3, Vec3);
    var_access_swizzle!(zxy, Vec3, Vec3);
    var_access_swizzle!(zyx, Vec3, Vec3);
}

// ==========================================
// VEC2 INITIAL ACCESS
// ==========================================
impl<T> Var<Vec2<T>> {
    var_access_swizzle_scalar!(x, Vec2);
    var_access_swizzle_scalar!(y, Vec2);

    var_access_swizzle!(xy, Vec2, Vec2);
    var_access_swizzle!(yx, Vec2, Vec2);
}
impl<'a, T> Var<Array1D<T>> {
    pub fn at(
        self,
        mv: impl Into<ShaderDSL<'a, Var<i32>>>,
    ) -> ShaderDSL<'a, TypedAccessExpr<Var<Array1D<T>>, T>> {
        mdo! {
            v <- mv.into();
            Free::Pure(TypedAccessExpr {
                v: VarAccessExpr::Array1DAccess(Box::new(VarAccessExpr::Var(self)), v),
                _marker: PhantomData,
            })
        }
    }
}
impl<'a, T> Var<Array2D<T>> {
    pub fn at(
        self,
        mv1: impl Into<ShaderDSL<'a, Var<i32>>>,
        mv2: impl Into<ShaderDSL<'a, Var<i32>>>,
    ) -> ShaderDSL<'a, TypedAccessExpr<Var<Array2D<T>>, T>> {
        let v1 = Arc::new(mv1.into());
        let v2 = Arc::new(mv2.into());
        _mdo_move! {
            [v1, v2]
            v1 <- (*v1).clone();
            v2 <- (*v2).clone();
            Free::Pure( TypedAccessExpr {
                v: VarAccessExpr::Array2DAccess(Box::new(VarAccessExpr::Var(self)), v1, v2),
                _marker: PhantomData,
            })
        }
    }
}
macro_rules! make_float4_op {
    ($a:expr $(,)?) => {{
        let a_val = Arc::new($a);
        _mdo_move! {
            ident <- _new_ident();
            v1 <- (*a_val).clone();
            _call_func_rt(FuncName::MakeFloat4, vec![v1.into()],
                ident
            )
        }
    }};
    ($a:expr, $b:expr $(,)?) => {{
        let a_val = Arc::new($a);
        let b_val = Arc::new($b);
        _mdo_move! {
            [a_val, b_val]
            ident <- _new_ident();
            v1 <- (*a_val).clone();
            v2 <- (*b_val).clone();
            _call_func_rt(FuncName::MakeFloat4, vec![v1.into(), v2.into()],
                ident
            )
        }
    }};
    ($a:expr, $b:expr, $c:expr $(,)?) => {{
        let a_val = Arc::new($a);
        let b_val = Arc::new($b);
        let c_val = Arc::new($c);
        _mdo_move! {
            [a_val, b_val, c_val]
            ident <- _new_ident();
            v1 <- (*a_val).clone();
            v2 <- (*b_val).clone();
            v3 <- (*c_val).clone();
            _call_func_rt(FuncName::MakeFloat4, vec![v1.into(), v2.into(), v3.into()],
                ident
            )
        }
    }};
    ($a:expr, $b:expr, $c:expr, $d:expr $(,)?) => {{
        let a_val = Arc::new($a);
        let b_val = Arc::new($b);
        let c_val = Arc::new($c);
        let d_val = Arc::new($d);
        _mdo_move! {
            [a_val, b_val, c_val, d_val]
            ident <- _new_ident();
            v1 <- (*a_val).clone();
            v2 <- (*b_val).clone();
            v3 <- (*c_val).clone();
            v4 <- (*d_val).clone();
            _call_func_rt(FuncName::MakeFloat4, vec![v1.into(), v2.into(), v3.into(), v4.into()],
                    ident
            )
        }
    }};
}
trait IntoFloat4<'a> {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>>;
}
impl<'a> IntoFloat4<'a>
    for (
        ShaderDSL<'a, Var<f32>>,
        ShaderDSL<'a, Var<f32>>,
        ShaderDSL<'a, Var<f32>>,
        ShaderDSL<'a, Var<f32>>,
    )
{
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_float4_op!(self.0, self.1, self.2, self.3)
    }
}
impl<'a> IntoFloat4<'a>
    for (
        ShaderDSL<'a, Var<Vec2<f32>>>,
        ShaderDSL<'a, Var<f32>>,
        ShaderDSL<'a, Var<f32>>,
    )
{
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_float4_op!(self.0, self.1, self.2)
    }
}
impl<'a> IntoFloat4<'a>
    for (
        ShaderDSL<'a, Var<f32>>,
        ShaderDSL<'a, Var<Vec2<f32>>>,
        ShaderDSL<'a, Var<f32>>,
    )
{
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_float4_op!(self.0, self.1, self.2)
    }
}
impl<'a> IntoFloat4<'a>
    for (
        ShaderDSL<'a, Var<f32>>,
        ShaderDSL<'a, Var<f32>>,
        ShaderDSL<'a, Var<Vec2<f32>>>,
    )
{
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_float4_op!(self.0, self.1, self.2)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<Vec2<f32>>>, ShaderDSL<'a, Var<Vec2<f32>>>) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_float4_op!(self.0, self.1)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<Vec3<f32>>>, ShaderDSL<'a, Var<f32>>) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_float4_op!(self.0, self.1)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<f32>>, ShaderDSL<'a, Var<Vec3<f32>>>) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_float4_op!(self.0, self.1)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<f32>>,) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_float4_op!(self.0)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<Vec4<f32>>>,) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_float4_op!(self.0)
    }
}

fn make_float4_impl<'a, T: IntoFloat4<'a>>(v: T) -> ShaderDSL<'a, Var<Vec4<f32>>> {
    v.into_float4()
}
trait IntoShaderVar<'a> {
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
impl<'a, T: 'a, CurrentT: Clone> IntoShaderVar<'a> for TypedAccessExpr<Var<T>, CurrentT> {
    type InnerT = CurrentT;
    fn in_context(self) -> ShaderDSL<'a, Var<CurrentT>> {
        self.into()
    }
}
impl<'a, T: 'a> IntoShaderVar<'a> for Var<T> {
    type InnerT = T;
    fn in_context(self) -> ShaderDSL<'a, Var<T>> {
        self.into()
    }
}
fn dsl_binary_op<'a, A, B, T>(op: FuncName, a: A, b: B) -> ShaderDSL<'a, Var<T>>
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
        ident <- _new_ident();
        v1 <- (*a_dsl).clone();
        v2 <- (*b_dsl).clone();
        _call_func_rt(
            (*op).clone(),
            vec![v1.into(), v2.into()],
            ident
        )
    }
}

macro_rules! impl_math_ops {
    ($trait:ident, $method:ident, $func_name:ident, $impl_type:ty) => {
        // 1. ShaderDSL + T (where T is anything that can become a ShaderDSL)
        impl<'a, T: Into<ShaderDSL<'a, Var<$impl_type>>>> $trait<T>
            for ShaderDSL<'a, Var<$impl_type>>
        {
            type Output = ShaderDSL<'a, Var<$impl_type>>;
            fn $method(self, rhs: T) -> Self::Output {
                dsl_binary_op(FuncName::$func_name, self, rhs)
            }
        }
        // 2. Var + ShaderDSL
        impl<'a> $trait<ShaderDSL<'a, Var<$impl_type>>> for Var<$impl_type> {
            type Output = ShaderDSL<'a, Var<$impl_type>>;
            fn $method(self, rhs: ShaderDSL<'a, Var<$impl_type>>) -> Self::Output {
                dsl_binary_op(FuncName::$func_name, self, rhs)
            }
        }
        // 3. TypedAccessExpr + ShaderDSL -> ShaderDSL
        impl<'a, B: 'a> $trait<ShaderDSL<'a, Var<$impl_type>>>
            for TypedAccessExpr<Var<B>, $impl_type>
        {
            type Output = ShaderDSL<'a, Var<$impl_type>>;
            fn $method(self, rhs: ShaderDSL<'a, Var<$impl_type>>) -> Self::Output {
                let lhs_dsl: ShaderDSL<'a, Var<$impl_type>> = self.into();
                dsl_binary_op(FuncName::$func_name, lhs_dsl, rhs)
            }
        }
    };
}
impl_math_ops!(Add, add, Add, f32);
impl_math_ops!(Add, add, Add, Vec2<f32>);
impl_math_ops!(Add, add, Add, Vec3<f32>);
impl_math_ops!(Add, add, Add, Vec4<f32>);
impl_math_ops!(Sub, sub, Sub, f32);
impl_math_ops!(Sub, sub, Sub, Vec2<f32>);
impl_math_ops!(Mul, mul, Mul, f32);
impl_math_ops!(Div, div, Div, f32);
#[macro_export]
macro_rules! make_float4 {
    ($($arg:expr),* $(,)?) => {
        make_float4_impl(( $($arg.in_context(),)* ))
    };
}

#[test]
fn test_dsl() {
    let program: ShaderDSL<'_, ()> = mdo! {
        val2 <- make_float4!(1.);
        val3 <- make_float4!(val2.x());
        _a <- val2 + val3.in_context();
        val4 <- make_float4!(val2.xy(), _a.xy());
    };
    println!("{}", _build_shader(program, String::from(""), 0));
}
