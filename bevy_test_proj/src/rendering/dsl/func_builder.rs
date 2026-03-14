use std::marker::PhantomData;

use crate::rendering::dsl::shader_dsl::IntoShaderVar;
use crate::rendering::dsl::vec_op::{TypedAccessExpr, Vec4};
use crate::{
    make_float4, mdo,
    rendering::dsl::{
        cast::WgslType,
        monad::{lift_f, Free, OwnedMonad},
        shader_dsl::{FuncName, ShaderDSL, ShaderDSLF, Var},
        vec_op::make_float4_impl,
        vec_op::Vec3,
    },
};

struct FunctionBuilder {
    shader: String,
    ident: i32,
}
impl FunctionBuilder {
    pub fn new() -> Self {
        FunctionBuilder {
            shader: String::from(""),
            ident: 0,
        }
    }
    fn _build_shader<'a, T>(mut self, v: ShaderDSL<'a, T>) -> String {
        match v {
            Free::Pure(a) => self.shader,
            Free::Free(step) => match *step {
                ShaderDSLF::NewIdent(next_f) => {
                    let a = self.ident;
                    self.ident += 1;
                    self._build_shader(next_f(a))
                }
                ShaderDSLF::BeginScope(next_prog) => {
                    self.shader += "\n{";
                    self._build_shader(next_prog)
                }
                ShaderDSLF::EndScope(next_prog) => {
                    self.shader += "\n}";
                    self._build_shader(next_prog)
                }
                ShaderDSLF::If(var, next_prog) => {
                    self.shader += format!("\nif(v{})", var.ident).as_str();
                    self._build_shader(next_prog)
                }
                ShaderDSLF::Else(next_prog) => {
                    self.shader += "\nelse";
                    self._build_shader(next_prog)
                }
                ShaderDSLF::Loop(t) => {
                    self.shader += "\nloop";
                    self._build_shader(t)
                }
                ShaderDSLF::Set(s, t) => {
                    self.shader += format!("\n{};", s).as_str();
                    self._build_shader(t)
                }
                ShaderDSLF::Call(rt, func_name, func_args, t) => {
                    let v: Vec<_> = func_args.iter().map(|v| v.to_string()).collect();
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
                    self.shader = format!(
                        "{}\n{}{};",
                        self.shader,
                        rt.map_or(String::from(""), |v| format!("let {} = ", v)),
                        call_fn(func_name)
                    );
                    self._build_shader(t)
                }
                ShaderDSLF::Continuing(t) => {
                    self.shader += "\ncontinuing";
                    self._build_shader(t)
                }
                ShaderDSLF::Return(s, t) => {
                    self.shader += &format!("\nreturn {};", s);
                    self._build_shader(t)
                }
            },
        }
    }
    pub fn vert<'a, In, Out, Body>(name: &str, body: Body) -> String
    where
        In: ShaderParam,
        Out: ShaderReturn + 'a + Clone,
        Body: Fn(In) -> ShaderDSL<'a, Out>, 
    {
        let mut ident = 0;
        let (param_strs, input_args) = In::get_wgsl_params(&mut ident);

        let program = body(input_args);

        let program_with_return = program.bind(move |out_val| {
            lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Return(out_val.to_return_string(), ()))
        });

        let body_str = FunctionBuilder::new()._build_shader(program_with_return); 

        format!(
            "@vertex\nfn {}({}) -> {}\n{{{}\n}}",
            name,
            param_strs.join(", "),
            Out::wgsl_return_decl(),
            body_str
        )
    }
    pub fn compute<'a, In, Body>(name: &str, workgroup_size: (u32, u32, u32), body: Body) -> String
    where
        In: ShaderParam,
        Body: Fn(In) -> ShaderDSL<'a, ()>, 
    {
        let mut ident = 0;

        let (param_strs, input_args) = In::get_wgsl_params(&mut ident);

        let program = body(input_args);

        let body_str = FunctionBuilder::new()._build_shader(program);

        format!(
            "@compute @workgroup_size({}, {}, {})\nfn {}({})\n{{{}\n}}",
            workgroup_size.0,
            workgroup_size.1,
            workgroup_size.2,
            name,
            param_strs.join(", "),
            body_str
        )
    }
    pub fn frag<'a, In, Out, Body>(name: &str, body: Body) -> String
    where
        In: ShaderParam,
        Out: ShaderReturn + 'a + Clone,
        Body: Fn(In) -> ShaderDSL<'a, Out>,
    {
        let mut ident = 0;
        let (param_strs, input_args) = In::get_wgsl_params(&mut ident);
        let program = body(input_args);
        let program_with_return = program.bind(move |out_val| {
            lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Return(out_val.to_return_string(), ()))
        });

        let builder = FunctionBuilder::new();
        let body_str = builder._build_shader(program_with_return);

        format!(
            "@fragment\nfn {}({}) -> {}\n{{{}\n}}",
            name,
            param_strs.join(", "),
            Out::wgsl_return_decl(),
            body_str
        )
    }
}

pub trait ShaderParam {
    
    fn get_wgsl_params(ident: &mut i32) -> (Vec<String>, Self);
}

#[derive(Copy, Clone)]
pub struct Location<const L: u32, T>(pub Var<T>);

impl<const L: u32, T: WgslType> ShaderParam for Location<L, T> {
    fn get_wgsl_params(ident: &mut i32) -> (Vec<String>, Self) {
        let id = *ident;
        *ident += 1;
        let decl = format!("@location({}) v{}: {}", L, id, T::wgsl_name());

        (
            vec![decl],
            Location(Var {
                ident: id,
                _marker: PhantomData,
            }),
        )
    }
}

pub trait BuiltInName {
    fn name() -> &'static str;
}
pub struct VertexIndex;
impl BuiltInName for VertexIndex {
    fn name() -> &'static str {
        "vertex_index"
    }
}
#[derive(Copy, Clone)]
pub struct Position;
impl BuiltInName for Position {
    fn name() -> &'static str {
        "position"
    }
}

#[derive(Copy, Clone)]
pub struct BuiltIn<B: BuiltInName, T>(pub Var<T>, pub PhantomData<B>);

impl<B: BuiltInName, T: WgslType> ShaderParam for BuiltIn<B, T> {
    fn get_wgsl_params(ident: &mut i32) -> (Vec<String>, Self) {
        let id = *ident;
        *ident += 1;
        let decl = format!("@builtin({}) v{}: {}", B::name(), id, T::wgsl_name());

        (
            vec![decl],
            BuiltIn(
                Var {
                    ident: id,
                    _marker: PhantomData,
                },
                PhantomData,
            ),
        )
    }
}
macro_rules! impl_shader_param_tuple {
    ($($P:ident),+) => {
        impl<$($P: ShaderParam),+> ShaderParam for ($($P,)+) {
            fn get_wgsl_params(ident: &mut i32) -> (Vec<String>, Self) {
                let mut decls = Vec::new();
                
                $(
                    let (mut d, $P) = $P::get_wgsl_params(ident);
                    decls.append(&mut d);
                )+
                (decls, ($($P,)+))
            }
        }
    };
}

impl_shader_param_tuple!(P1);
impl_shader_param_tuple!(P1, P2);
impl_shader_param_tuple!(P1, P2, P3);

pub trait ShaderReturn {
    fn wgsl_return_decl() -> String;

    fn to_return_string(&self) -> String;
}

impl<const L: u32, T> Location<L, T> {
    pub fn new(v: Var<T>) -> Self {
        Self(v)
    }
}

impl<const L: u32, T: WgslType> ShaderReturn for Location<L, T> {
    fn wgsl_return_decl() -> String {
        format!("@location({}) {}", L, T::wgsl_name())
    }
    fn to_return_string(&self) -> String {
        self.0.to_string() 
    }
}

impl<B: BuiltInName, T> BuiltIn<B, T> {
    pub fn new(v: Var<T>) -> Self {
        Self(v, PhantomData)
    }
}

impl<B: BuiltInName, T: WgslType> ShaderReturn for BuiltIn<B, T> {
    fn wgsl_return_decl() -> String {
        format!("@builtin({}) {}", B::name(), T::wgsl_name())
    }
    fn to_return_string(&self) -> String {
        self.0.to_string()
    }
}
pub struct GlobalInvocationId;
impl BuiltInName for GlobalInvocationId {
    fn name() -> &'static str {
        "global_invocation_id"
    }
}

pub struct LocalInvocationId;
impl BuiltInName for LocalInvocationId {
    fn name() -> &'static str {
        "local_invocation_id"
    }
}

pub struct WorkgroupId;
impl BuiltInName for WorkgroupId {
    fn name() -> &'static str {
        "workgroup_id"
    }
}

pub struct LocalInvocationIndex;
impl BuiltInName for LocalInvocationIndex {
    fn name() -> &'static str {
        "local_invocation_index"
    }
}
pub trait ShaderStruct {
    fn wgsl_struct_decl() -> String; 
}

pub struct EmptyShader;
pub struct ShaderCode<State, Uniforms> {
    structs: String,
    globals: String,
    functions: String,
    uniforms: Uniforms,
    _state: PhantomData<State>,
}

impl ShaderCode<EmptyShader, Nil> {
    pub fn new() -> Self {
        Self {
            structs: String::new(),
            globals: String::new(),
            functions: String::new(),
            uniforms: Nil,
            _state: PhantomData,
        }
    }
}

impl<State, Uniforms> ShaderCode<State, Uniforms> {
    pub fn uniform<Key, T: WgslType + ShaderStruct>(
        mut self,
        group: u32,
        binding: u32,
        var_name: &str,
    ) -> ShaderCode<State, Cons<Key, T, Uniforms>> {
        
        let struct_decl = T::wgsl_struct_decl();
        if !self.structs.contains(&struct_decl) {
            self.structs.push_str(&struct_decl);
            self.structs.push_str("\n\n");
        }
        let global_decl = format!(
            "@group({}) @binding({}) var<uniform> {}: {};\n",
            group, binding, var_name, T::wgsl_name()
        );
        self.globals.push_str(&global_decl);

        let new_var = Var::new_global(var_name);

        ShaderCode {
            structs: self.structs,
            globals: self.globals,
            functions: self.functions,
            _state: PhantomData,
            uniforms: Cons {
                var: new_var,
                _key: PhantomData,
                tail: self.uniforms,
            },
        }
    }
}
impl<Uniforms> ShaderCode<EmptyShader, Uniforms> {
    pub fn build_pipeline<F, FinalState>(self, builder_closure: F) -> String
    where
        F: FnOnce(ShaderCode<EmptyShader, Nil>, &Uniforms) -> ShaderCode<FinalState, Nil>,
    {
        let clean_builder = ShaderCode {
            structs: self.structs,
            globals: self.globals,
            functions: self.functions,
            uniforms: Nil,
            _state: PhantomData,
        };

        let final_builder = builder_closure(clean_builder, &self.uniforms);

        format!(
            "{}\n{}\n{}",
            final_builder.structs, final_builder.globals, final_builder.functions
        )
    }
}

pub struct Nil;
pub struct Cons<K, V, Tail> {
    pub var: Var<V>,
    _key: PhantomData<K>,
    pub tail: Tail,
}

pub struct Here;
pub struct There<T>(PhantomData<T>);

pub trait GetUniform<IsFound, Key> {
    type Type;
    fn get(&self, _key: Key) -> Var<Self::Type>;
}

impl<Key, Val, Tail> GetUniform<Here, Key> for Cons<Key, Val, Tail> 
where 
    Var<Val>: Clone 
{
    type Type = Val;
    fn get(&self, _key: Key) -> Var<Self::Type> {
        self.var.clone()
    }
}

impl<K, Key, Val, Tail, Rest> GetUniform<There<Rest>, Key> for Cons<K, Val, Tail>
where
    Tail: GetUniform<Rest, Key>,
{
    type Type = Tail::Type;
    fn get(&self, key: Key) -> Var<Self::Type> {
        self.tail.get(key)
    }
}

#[test]
fn test_shader_builder() {
    struct Camera {
        view_proj: Vec4<f32>,
        position: Vec3<f32>,
    }
    impl WgslType for Camera {
    fn wgsl_name() -> &'static str { "Camera" }
    }

    impl ShaderStruct for Camera {
        fn wgsl_struct_decl() -> String {
            "struct Camera {\n    view_proj: mat4x4<f32>,\n    position: vec3<f32>,\n}".to_string()
        }
    }

}