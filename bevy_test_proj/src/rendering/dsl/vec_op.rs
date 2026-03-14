use std::{
    marker::PhantomData,
    ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub},
    sync::Arc,
};

use crate::rendering::dsl::monad::OwnedMonad;
use crate::{
    _mdo_move, mdo,
    rendering::dsl::{
        monad::Free,
        shader_dsl::{
            FuncArg, FuncName, IntoShaderVar, ShaderDSL, _call_func_rt, _new_ident, _set_statement_let,
        },
    },
};

use super::shader_dsl::Var;

#[derive(Clone)]
pub enum VarAccessExpr<T> {
    // TODO)): turn this into compile-time
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
#[derive(Clone)]
pub(super) struct TypedAccessExpr<T, CurrentT> {
    v: VarAccessExpr<T>,
    _marker: PhantomData<CurrentT>,
}
impl<'a, T: 'a, CurrentT: Clone> IntoShaderVar<'a> for TypedAccessExpr<Var<T>, CurrentT> {
    type InnerT = CurrentT;
    fn in_context(self) -> ShaderDSL<'a, Var<CurrentT>> {
        self.into()
    }
}
impl<T, CurrentT> ToString for TypedAccessExpr<Var<T>, CurrentT> {
    fn to_string(&self) -> String {
        access_to_string(&self.v)
    }
}
#[derive(Clone, Copy)]
pub struct Vec4<T>(PhantomData<T>);
#[derive(Clone, Copy)]
pub struct Vec3<T>(PhantomData<T>);
#[derive(Clone, Copy)]
pub struct Vec2<T>(PhantomData<T>);
#[derive(Clone, Copy)]
pub struct Array1D<T>(PhantomData<T>);
#[derive(Clone, Copy)]
struct Array2D<T>(PhantomData<T>);
#[derive(Clone, Copy)]
pub struct Mat4x4<T>(PhantomData<T>);
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

impl<T> Var<Vec2<T>> {
    var_access_swizzle_scalar!(x, Vec2);
    var_access_swizzle_scalar!(y, Vec2);

    var_access_swizzle!(xy, Vec2, Vec2);
    var_access_swizzle!(yx, Vec2, Vec2);
}
impl<'a, T: 'a, CurrentT: Clone> From<TypedAccessExpr<Var<T>, CurrentT>>
    for ShaderDSL<'a, Var<CurrentT>>
{
    fn from(val: TypedAccessExpr<Var<T>, CurrentT>) -> Self {
        mdo! {
            new_ident <- _new_ident();
            _set_statement_let(new_ident.to_string(), val.to_string());
            Free::Pure(new_ident)
        }
    }
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
impl_math_ops!(Add, add, Add, i32);
impl_math_ops!(Add, add, Add, Vec2<f32>);
impl_math_ops!(Add, add, Add, Vec3<f32>);
impl_math_ops!(Add, add, Add, Vec4<f32>);
impl_math_ops!(Sub, sub, Sub, f32);
impl_math_ops!(Sub, sub, Sub, Vec2<f32>);
impl_math_ops!(Mul, mul, Mul, f32);
impl_math_ops!(Div, div, Div, f32);
impl_math_ops!(Rem, rem, Rem, f32);
impl_math_ops!(Rem, rem, Rem, i32);
impl_math_ops!(Rem, rem, Rem, u32);
impl_math_ops!(BitAnd, bitand, BitAnd, u32);
impl_math_ops!(BitAnd, bitand, BitAnd, i32);
impl_math_ops!(BitOr, bitor, BitOr, u32);
impl_math_ops!(BitOr, bitor, BitOr, i32);
impl_math_ops!(BitXor, bitxor, BitXor, u32);
impl_math_ops!(BitXor, bitxor, BitXor, i32);
impl_math_ops!(Shl, shl, Shl, u32);
impl_math_ops!(Shr, shr, Shr, u32);
fn dsl_unary_op<'a, A, T>(op: FuncName, a: A) -> ShaderDSL<'a, Var<T>>
where
    A: Into<ShaderDSL<'a, Var<T>>>,
    Var<T>: Into<FuncArg>,
{
    let a_dsl = Arc::new(a.into());
    let op = Arc::new(op);

    _mdo_move! {
        [a_dsl, op]
        ident <- _new_ident();
        v1 <- (*a_dsl).clone();
        _call_func_rt(
            (*op).clone(),
            vec![v1.into()],
            ident
        )
    }
}

macro_rules! impl_unary_ops {
    ($trait:ident, $method:ident, $func_name:ident, $impl_type:ty) => {
        impl<'a> $trait for ShaderDSL<'a, Var<$impl_type>> {
            type Output = ShaderDSL<'a, Var<$impl_type>>;
            fn $method(self) -> Self::Output {
                dsl_unary_op(FuncName::$func_name, self)
            }
        }
    };
}
impl_unary_ops!(Neg, neg, Neg, f32);
impl_unary_ops!(Neg, neg, Neg, i32);
impl_unary_ops!(Neg, neg, Neg, Vec2<f32>);
impl_unary_ops!(Neg, neg, Neg, Vec3<f32>);
impl_unary_ops!(Neg, neg, Neg, Vec4<f32>);

impl_unary_ops!(Not, not, Not, bool);

macro_rules! make_vecf_op {
    ($t:expr, $a:expr $(,)?) => {{
        let a_val = Arc::new($a);
        _mdo_move! {
            ident <- _new_ident();
            v1 <- (*a_val).clone();
            _call_func_rt($t, vec![v1.into()],
                ident
            )
        }
    }};
    ($t:expr, $a:expr, $b:expr $(,)?) => {{
        let a_val = Arc::new($a);
        let b_val = Arc::new($b);
        _mdo_move! {
            [a_val, b_val]
            ident <- _new_ident();
            v1 <- (*a_val).clone();
            v2 <- (*b_val).clone();
            _call_func_rt($t, vec![v1.into(), v2.into()],
                ident
            )
        }
    }};
    ($t:expr, $a:expr, $b:expr, $c:expr $(,)?) => {{
        let a_val = Arc::new($a);
        let b_val = Arc::new($b);
        let c_val = Arc::new($c);
        _mdo_move! {
            [a_val, b_val, c_val]
            ident <- _new_ident();
            v1 <- (*a_val).clone();
            v2 <- (*b_val).clone();
            v3 <- (*c_val).clone();
            _call_func_rt($t, vec![v1.into(), v2.into(), v3.into()],
                ident
            )
        }
    }};
    ($t:expr, $a:expr, $b:expr, $c:expr, $d:expr $(,)?) => {{
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
            _call_func_rt($t, vec![v1.into(), v2.into(), v3.into(), v4.into()],
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
        make_vecf_op!(FuncName::MakeFloat4, self.0, self.1, self.2, self.3)
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
        make_vecf_op!(FuncName::MakeFloat4, self.0, self.1, self.2)
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
        make_vecf_op!(FuncName::MakeFloat4, self.0, self.1, self.2)
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
        make_vecf_op!(FuncName::MakeFloat4, self.0, self.1, self.2)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<Vec2<f32>>>, ShaderDSL<'a, Var<Vec2<f32>>>) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_vecf_op!(FuncName::MakeFloat4, self.0, self.1)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<Vec3<f32>>>, ShaderDSL<'a, Var<f32>>) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_vecf_op!(FuncName::MakeFloat4, self.0, self.1)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<f32>>, ShaderDSL<'a, Var<Vec3<f32>>>) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_vecf_op!(FuncName::MakeFloat4, self.0, self.1)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<f32>>,) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_vecf_op!(FuncName::MakeFloat4, self.0)
    }
}
impl<'a> IntoFloat4<'a> for (ShaderDSL<'a, Var<Vec4<f32>>>,) {
    fn into_float4(self) -> ShaderDSL<'a, Var<Vec4<f32>>> {
        make_vecf_op!(FuncName::MakeFloat4, self.0)
    }
}

pub fn make_float4_impl<'a, T: IntoFloat4<'a>>(v: T) -> ShaderDSL<'a, Var<Vec4<f32>>> {
    v.into_float4()
}
#[macro_export]
macro_rules! make_float4 {
    ($($arg:expr),* $(,)?) => {
        make_float4_impl(( $($arg.in_context(),)* ))
    };
}
trait IntoFloat3<'a> {
    fn into_float3(self) -> ShaderDSL<'a, Var<Vec3<f32>>>;
}
impl<'a> IntoFloat3<'a> for (ShaderDSL<'a, Var<f32>>, ShaderDSL<'a, Var<f32>>, ShaderDSL<'a, Var<f32>>) {
    fn into_float3(self) -> ShaderDSL<'a, Var<Vec3<f32>>> {
        make_vecf_op!(FuncName::MakeFloat3, self.0, self.1, self.2)
    }
}
impl<'a> IntoFloat3<'a> for (ShaderDSL<'a, Var<Vec2<f32>>>, ShaderDSL<'a, Var<f32>>) {
    fn into_float3(self) -> ShaderDSL<'a, Var<Vec3<f32>>> {
        make_vecf_op!(FuncName::MakeFloat3, self.0, self.1)
    }
}
impl<'a> IntoFloat3<'a> for (ShaderDSL<'a, Var<f32>>, ShaderDSL<'a, Var<Vec2<f32>>>) {
    fn into_float3(self) -> ShaderDSL<'a, Var<Vec3<f32>>> {
        make_vecf_op!(FuncName::MakeFloat3, self.0, self.1)
    }
}
impl<'a> IntoFloat3<'a> for (ShaderDSL<'a, Var<f32>>,) {
    fn into_float3(self) -> ShaderDSL<'a, Var<Vec3<f32>>> {
        make_vecf_op!(FuncName::MakeFloat3, self.0)
    }
}
impl<'a> IntoFloat3<'a> for (ShaderDSL<'a, Var<Vec3<f32>>>,) {
    fn into_float3(self) -> ShaderDSL<'a, Var<Vec3<f32>>> {
        make_vecf_op!(FuncName::MakeFloat3, self.0)
    }
}

pub fn make_float3_impl<'a, T: IntoFloat3<'a>>(v: T) -> ShaderDSL<'a, Var<Vec3<f32>>> {
    v.into_float3()
}

#[macro_export]
macro_rules! make_float3 {
    ($($arg:expr),* $(,)?) => {
        make_float3_impl(( $($arg.in_context(),)* ))
    };
}
trait IntoFloat2<'a> {
    fn into_float2(self) -> ShaderDSL<'a, Var<Vec2<f32>>>;
}
impl<'a> IntoFloat2<'a> for (ShaderDSL<'a, Var<f32>>, ShaderDSL<'a, Var<f32>>) {
    fn into_float2(self) -> ShaderDSL<'a, Var<Vec2<f32>>> {
        make_vecf_op!(FuncName::MakeFloat2, self.0, self.1)
    }
}
impl<'a> IntoFloat2<'a> for (ShaderDSL<'a, Var<f32>>,) {
    fn into_float2(self) -> ShaderDSL<'a, Var<Vec2<f32>>> {
        make_vecf_op!(FuncName::MakeFloat2, self.0)
    }
}
impl<'a> IntoFloat2<'a> for (ShaderDSL<'a, Var<Vec2<f32>>>,) {
    fn into_float2(self) -> ShaderDSL<'a, Var<Vec2<f32>>> {
        make_vecf_op!(FuncName::MakeFloat2, self.0)
    }
}

pub fn make_float2_impl<'a, T: IntoFloat2<'a>>(v: T) -> ShaderDSL<'a, Var<Vec2<f32>>> {
    v.into_float2()
}

#[macro_export]
macro_rules! make_float2 {
    ($($arg:expr),* $(,)?) => {
        make_float2_impl(( $($arg.in_context(),)* ))
    };
}

fn dsl_mixed_binary_op<'a, A, B, L : 'a, R : 'a, Out>(op: FuncName, a: A, b: B) -> ShaderDSL<'a, Var<Out>>
where
    A: Into<ShaderDSL<'a, Var<L>>>,
    B: Into<ShaderDSL<'a, Var<R>>>,
    Var<L>: Into<FuncArg>,
    Var<R>: Into<FuncArg>,
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

macro_rules! impl_mixed_math_ops {
    ($trait:ident, $method:ident, $func_name:ident, $vec_type:ident, $scalar_type:ident) => {
        // --------------------------------------------------------
        // VEC OP SCALAR -> VEC
        // --------------------------------------------------------
        
        // 1. ShaderDSL<Vec> + ShaderDSL<Scalar>
        impl<'a> $trait<ShaderDSL<'a, Var<$scalar_type>>> for ShaderDSL<'a, Var<$vec_type<f32>>> {
            type Output = ShaderDSL<'a, Var<$vec_type<f32>>>;
            fn $method(self, rhs: ShaderDSL<'a, Var<$scalar_type>>) -> Self::Output {
                dsl_mixed_binary_op(FuncName::$func_name, self, rhs)
            }
        }

        // 2. ShaderDSL<Vec> + Var<Scalar>
        impl<'a> $trait<Var<$scalar_type>> for ShaderDSL<'a, Var<$vec_type<f32>>> {
            type Output = ShaderDSL<'a, Var<$vec_type<f32>>>;
            fn $method(self, rhs: Var<$scalar_type>) -> Self::Output {
                dsl_mixed_binary_op(FuncName::$func_name, self, rhs)
            }
        }
        
        // 3. Var<Vec> + ShaderDSL<Scalar>
        impl<'a> $trait<ShaderDSL<'a, Var<$scalar_type>>> for Var<$vec_type<f32>> {
            type Output = ShaderDSL<'a, Var<$vec_type<f32>>>;
            fn $method(self, rhs: ShaderDSL<'a, Var<$scalar_type>>) -> Self::Output {
                dsl_mixed_binary_op(FuncName::$func_name, self, rhs)
            }
        }

        // 4. TypedAccessExpr<Vec> + ShaderDSL<Scalar>
        impl<'a, B: 'a> $trait<ShaderDSL<'a, Var<$scalar_type>>> for TypedAccessExpr<Var<B>, $vec_type<f32>> {
            type Output = ShaderDSL<'a, Var<$vec_type<f32>>>;
            fn $method(self, rhs: ShaderDSL<'a, Var<$scalar_type>>) -> Self::Output {
                let lhs_dsl: ShaderDSL<'a, Var<$vec_type<f32>>> = self.into();
                dsl_mixed_binary_op(FuncName::$func_name, lhs_dsl, rhs)
            }
        }

        // --------------------------------------------------------
        // SCALAR OP VEC -> VEC
        // --------------------------------------------------------

        // 5. ShaderDSL<Scalar> + ShaderDSL<Vec>
        impl<'a> $trait<ShaderDSL<'a, Var<$vec_type<f32>>>> for ShaderDSL<'a, Var<$scalar_type>> {
            type Output = ShaderDSL<'a, Var<$vec_type<f32>>>;
            fn $method(self, rhs: ShaderDSL<'a, Var<$vec_type<f32>>>) -> Self::Output {
                dsl_mixed_binary_op(FuncName::$func_name, self, rhs)
            }
        }

        // 6. ShaderDSL<Scalar> + Var<Vec>
        impl<'a> $trait<Var<$vec_type<f32>>> for ShaderDSL<'a, Var<$scalar_type>> {
            type Output = ShaderDSL<'a, Var<$vec_type<f32>>>;
            fn $method(self, rhs: Var<$vec_type<f32>>) -> Self::Output {
                dsl_mixed_binary_op(FuncName::$func_name, self, rhs)
            }
        }

        // 7. Var<Scalar> + ShaderDSL<Vec>
        impl<'a> $trait<ShaderDSL<'a, Var<$vec_type<f32>>>> for Var<$scalar_type> {
            type Output = ShaderDSL<'a, Var<$vec_type<f32>>>;
            fn $method(self, rhs: ShaderDSL<'a, Var<$vec_type<f32>>>) -> Self::Output {
                dsl_mixed_binary_op(FuncName::$func_name, self, rhs)
            }
        }

        // 8. TypedAccessExpr<Scalar> + ShaderDSL<Vec>
        impl<'a, B: 'a> $trait<ShaderDSL<'a, Var<$vec_type<f32>>>> for TypedAccessExpr<Var<B>, $scalar_type> {
            type Output = ShaderDSL<'a, Var<$vec_type<f32>>>;
            fn $method(self, rhs: ShaderDSL<'a, Var<$vec_type<f32>>>) -> Self::Output {
                let lhs_dsl: ShaderDSL<'a, Var<$scalar_type>> = self.into();
                dsl_mixed_binary_op(FuncName::$func_name, lhs_dsl, rhs)
            }
        }
    };
}

impl_mixed_math_ops!(Mul, mul, Mul, Vec2, f32);
impl_mixed_math_ops!(Mul, mul, Mul, Vec3, f32);
impl_mixed_math_ops!(Mul, mul, Mul, Vec4, f32);

impl_mixed_math_ops!(Div, div, Div, Vec2, f32);
impl_mixed_math_ops!(Div, div, Div, Vec3, f32);
impl_mixed_math_ops!(Div, div, Div, Vec4, f32);