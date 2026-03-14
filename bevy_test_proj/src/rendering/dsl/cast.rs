use std::sync::Arc;
use crate::rendering::dsl::monad::OwnedMonad;

use crate::{_mdo_move, mdo, rendering::dsl::{shader_dsl::{_call_func_rt, _new_ident, FuncArg, FuncName, ShaderDSL, Var}, vec_op::{ TypedAccessExpr, Vec2, Vec3, Vec4 }}};

pub trait WgslType {
    fn wgsl_name() -> &'static str;
}

impl WgslType for f32 { fn wgsl_name() -> &'static str { "f32" } }
impl WgslType for u32 { fn wgsl_name() -> &'static str { "u32" } }
impl WgslType for i32 { fn wgsl_name() -> &'static str { "i32" } }
impl WgslType for Vec2<f32> { fn wgsl_name() -> &'static str { "vec2f" } }
impl WgslType for Vec3<f32> { fn wgsl_name() -> &'static str { "vec3f" } }
impl WgslType for Vec4<f32> { fn wgsl_name() -> &'static str { "vec4f" } }
impl WgslType for Vec2<u32> { fn wgsl_name() -> &'static str { "vec2u" } }
impl WgslType for Vec3<u32> { fn wgsl_name() -> &'static str { "vec3u" } }
impl WgslType for Vec4<u32> { fn wgsl_name() -> &'static str { "vec4u" } }
impl<T: WgslType> WgslType for Var<T> { fn wgsl_name() -> &'static str { T::wgsl_name() } }
pub trait ShaderCast<'a, T> {
    fn cast<U: WgslType>(self) -> ShaderDSL<'a, Var<U>>;
}

impl<'a, T : 'a> ShaderCast<'a, T> for Var<T> 
    where Var<T> : Into<FuncArg>
{
    fn cast<U: WgslType>(self) -> ShaderDSL<'a, Var<U>> {
        mdo! {
            ident <- _new_ident();
            _call_func_rt(FuncName::Cast(U::wgsl_name()), vec![self.into()], ident)
        }
    }
}

impl<'a, T> ShaderCast<'a, T> for ShaderDSL<'a, Var<T>> 
where 
    Var<T>: Into<FuncArg> 
{
    fn cast<U: WgslType>(self) -> ShaderDSL<'a, Var<U>> {
        let v_dsl = Arc::new(self);
        
        _mdo_move! {
            [v_dsl]
            ident <- _new_ident();
            v <- (*v_dsl).clone();
            _call_func_rt(FuncName::Cast(U::wgsl_name()), vec![v.into()], ident)
        }
    }
}

impl<'a, Base : 'a, T : Clone + 'a> ShaderCast<'a, T> for TypedAccessExpr<Var<Base>, T> 
where 
    Var<T>: Into<FuncArg> 
{
    fn cast<U: WgslType>(self) -> ShaderDSL<'a, Var<U>> {
        let v_dsl: ShaderDSL<'a, Var<T>> = self.into();
        v_dsl.cast::<U>()
    }
}
