pub mod monad;
pub mod shader_dsl;
pub mod vec_op;
pub mod builtin_func;
pub mod cast;
pub mod func_builder;

pub use monad::{Free, OwnedFunctor, OwnedApplicative, OwnedMonad, MonadFree, CloneWrapped, lift_f, retract};

pub use shader_dsl::{Var, VarType, ShaderDSL, if_, while_, IntoShaderVar, FuncArg, FuncName, _new_ident, _call_func_rt};

pub use vec_op::{Vec2, Vec3, Vec4, Array1D, Mat4x4, VarAccessExpr, TypedAccessExpr, 
                 make_float4_impl, make_float3_impl, make_float2_impl};

pub use builtin_func::{
    ShaderCmp, ShaderLogic, ShaderLift1, ShaderLift2, ShaderLift3, ShaderLift4,
    ForM, ForDSL, LVal,
    normalize, length, dot, cross, pow, distance, reflect, mix, clamp, set, abs, sin, cos, tan, sqrt, select, max, min, sign, return_, radians
};

pub use cast::{WgslType, ShaderCast};

pub use func_builder::{
    ShaderCode, EmptyShader,
    
    ShaderParam, Location, BuiltIn, ShaderReturn, ShaderClosure,
    
    Nil, Cons, Here, There, GetUniform,
    
    ReadWrite, Read, Write, ToWgslAccessScope,
    
    GlobalInvocationId, LocalInvocationId, WorkgroupId, LocalInvocationIndex,
    Position, VertexIndex,
    
    ShaderFn, ShaderFnParam, ShaderFnReturn, ShaderFnClosure, define_fn,
    
    ShaderStruct,
};

pub use crate::{mdo, _mdo_move, make_float4, make_float3, make_float2};