use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

use shader_macros::ShaderStruct;

use crate::{_mdo_move, make_float2, make_float3};
use crate::rendering::dsl::builtin_func::set;
use crate::rendering::dsl::shader_dsl::{_call_func, _call_func_rt, _new_ident, IntoShaderVar, VarType};
use crate::rendering::dsl::vec_op::{Array1D, Mat4x4, TypedAccessExpr, VarAccessExpr, Vec4};
use crate::{
    make_float4, mdo,
    rendering::dsl::{
        cast::WgslType,
        monad::{lift_f, Free, OwnedMonad},
        shader_dsl::{FuncName, FuncArg, ShaderDSL, ShaderDSLF, Var},
        vec_op::make_float4_impl,
        vec_op::make_float3_impl,
        vec_op::make_float2_impl,
        vec_op::Vec3,
    },
};

type FunctionMap = HashMap<&'static str, Arc<String>>;
struct FunctionBuilder {
    shader: String,
    ident: i32,
    deps: HashMap<&'static str, Arc<String>>
}
impl FunctionBuilder {
    pub fn new() -> Self {
        FunctionBuilder {
            shader: String::from(""),
            ident: 0,
            deps: HashMap::new()
        }
    }
    pub fn new_with_ident(ident : i32) -> Self {
        FunctionBuilder {
            shader: String::from(""),
            ident: ident,
            deps: HashMap::new()
        }
    }
    fn _build_shader<'a, T>(&mut self, v: ShaderDSL<'a, T>) -> String {
        match v {
            Free::Pure(a) => self.shader.clone(),
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
                    let mut call_fn = |fn_name| match fn_name {
                        FuncName::MakeFloat4 => format!("vec4f({})", v.join(",")),
                        FuncName::MakeFloat3 => format!("vec3f({})", v.join(",")),
                        FuncName::MakeFloat2 => format!("vec2f({})", v.join(",")),
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
                        FuncName::Continue => "continue".to_string(), // TODO)) refactor this
                        FuncName::NormalFunctionInvoke(s) => format!("{}({})", s, v.join(",")),
                        FuncName::Cast(t) => format!("{}({})", t, v[0]),
                        FuncName::CallExtFn(t, content, sub_deps) => {
                            self.deps.extend((*sub_deps).clone().into_iter());
                            if !self.deps.contains_key(t){
                                self.deps.insert(t, content);
                            }
                            format!("{}({})", t, v.join(","))
                        }
                        FuncName::Neg => format!("-({})", v[0]),
                        FuncName::Not => format!("!({})", v[0]),
                        FuncName::BiOp(s) => format!("({} {} {})", v[0], s, v[1]),
                    };
                    self.shader = format!(
                        "{}\n{}{};",
                        self.shader,
                        rt.map_or(String::from(""), |v| format!("var {} = ", v)),
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
    pub fn vert<'a, Args, Out, Body>(name: &str, body: Body) -> (FunctionMap, String)
    where
        Out: ShaderReturn + 'a + Clone,
        Body: ShaderClosure<'a, Args, Out>, 
    {
        let mut ident = 0;
        let (param_strs, program) = body.build_and_evaluate(&mut ident);
        let program_with_return = program.bind(move |out_val| {
            lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Return(out_val.to_return_string(), ()))
        });

        let mut fb = FunctionBuilder::new_with_ident(ident);
        let body_str = fb._build_shader(program_with_return); 

        (fb.deps, format!(
            "@vertex\nfn {}({}) -> {}\n{{{}\n}}",
            name,
            param_strs.join(", "),
            Out::wgsl_return_decl(),
            body_str
        ))
    }
    pub fn compute<'a, Args, Body>(name: &str, workgroup_size: (u32, u32, u32), body: Body) -> (FunctionMap, String)
    where
        Body: ShaderClosure<'a, Args, ()>
    {
        let mut ident = 0;
        let (param_strs, program) = body.build_and_evaluate(&mut ident);
        
        let mut fb = FunctionBuilder::new_with_ident(ident);
        let body_str = fb._build_shader(program);

        (fb.deps, format!(
            "@compute @workgroup_size({}, {}, {})\nfn {}({})\n{{{}\n}}",
            workgroup_size.0,
            workgroup_size.1,
            workgroup_size.2,
            name,
            param_strs.join(", "),
            body_str
        ))
    }
    pub fn frag<'a, Args, Out, Body>(name: &str, body: Body) -> (FunctionMap, String)
    where
        Out: ShaderReturn + 'a + Clone,
        Body: ShaderClosure<'a, Args, Out>,
    {
        let mut ident = 0;
        let (param_strs, program) = body.build_and_evaluate(&mut ident);
        let program_with_return = program.bind(move |out_val| {
            lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Return(out_val.to_return_string(), ()))
        });

        let mut fb = FunctionBuilder::new_with_ident(ident);
        let body_str = fb._build_shader(program_with_return);

        (fb.deps, format!(
            "@fragment\nfn {}({}) -> {}\n{{{}\n}}",
            name,
            param_strs.join(", "),
            Out::wgsl_return_decl(),
            body_str
        ))
    }
}

impl Default for FunctionBuilder {
    fn default() -> Self {
        Self::new()
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
                t: VarType::Local
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
                    t: VarType::Local
                },
                PhantomData,
            ),
        )
    }
}
pub trait ShaderClosure<'a, Args, Out> {
    fn build_and_evaluate(self, ident: &mut i32) -> (Vec<String>, ShaderDSL<'a, Out>);
}
impl<'a, Out : 'a, Func> ShaderClosure<'a, (), Out> for Func
where
    Func: Fn() -> ShaderDSL<'a, Out>,
{
    fn build_and_evaluate(self, _ident: &mut i32) -> (Vec<String>, ShaderDSL<'a, Out>) {
        (Vec::new(), self())
    }
}
macro_rules! impl_shader_closure {
    ( $($Param:ident),* ) => {
        #[allow(non_snake_case)]
        // Note the tuple `($($Param,)*)` right here!
        impl<'a, Out : 'a, Func, $($Param),*> ShaderClosure<'a, ($($Param,)*), Out> for Func
        where
            $($Param: ShaderParam,)*
            Func: FnOnce($($Param),*) -> ShaderDSL<'a, Out>,
        {
            fn build_and_evaluate(self, ident: &mut i32) -> (Vec<String>, ShaderDSL<'a, Out>) {
                let mut all_decls = Vec::new();
                
                $(
                    let (mut decls, $Param) = $Param::get_wgsl_params(ident);
                    all_decls.append(&mut decls);
                )*
                
                (all_decls, self($($Param),*))
            }
        }
    };
}

impl_shader_closure!(A);
impl_shader_closure!(A, B);
impl_shader_closure!(A, B, C);
impl_shader_closure!(A, B, C, D);
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
    g_ident: i32,
    structs: String,
    globals: String,
    ext_functions: HashMap<&'static str, Arc<String>>,
    functions: String,
    uniforms: Uniforms,
    _state: PhantomData<State>,
}

impl ShaderCode<EmptyShader, Nil> {
    pub fn new() -> Self {
        Self {
            g_ident: 0,
            structs: String::new(),
            globals: String::new(),
            ext_functions: HashMap::new(),
            functions: String::new(),
            uniforms: Nil,
            _state: PhantomData,
        }
    }
}

pub struct ReadWrite;
pub struct Read;
pub struct Write;
pub trait ToWgslAccessScope {
    fn to_name() -> &'static str;
}
impl ToWgslAccessScope for Read {
    fn to_name() -> &'static str {
        "read"
    }
}
impl ToWgslAccessScope for Write {
    fn to_name() -> &'static str {
        "write"
    }
}
impl ToWgslAccessScope for ReadWrite {
    fn to_name() -> &'static str {
        "read_write"
    }
}

impl<State, Uniforms> ShaderCode<State, Uniforms> {
    pub fn uniform<Key, T: WgslType + ShaderStruct>(
        mut self,
        group: u32,
        binding: u32,
    ) -> ShaderCode<State, Cons<Key, T, Uniforms>> {
        
        let struct_decl = T::wgsl_struct_decl();
        if !self.structs.contains(&struct_decl) {
            self.structs.push_str(&struct_decl);
            self.structs.push_str("\n\n");
        }
        let global_decl = format!(
            "@group({}) @binding({}) var<uniform> g{}: {};\n",
            group, binding, self.g_ident, T::wgsl_name()
        );
        self.globals.push_str(&global_decl);

        let new_var = Var { ident: self.g_ident, _marker: PhantomData, t: VarType::Global };

        ShaderCode {
            g_ident: self.g_ident + 1,
            structs: self.structs,
            globals: self.globals,
            ext_functions: self.ext_functions,
            functions: self.functions,
            _state: PhantomData,
            uniforms: Cons {
                var: new_var,
                _key: PhantomData,
                tail: self.uniforms,
            },
        }
    }
    pub fn storage<Key, T: WgslType, RT, Access : ToWgslAccessScope>(
        mut self,
        group: u32,
        binding: u32,
    ) -> ShaderCode<State, Cons<Key, RT, Uniforms>> {
        
        // 1. Write the storage buffer declaration using `array<T>`
        let global_decl = format!(
            "@group({}) @binding({}) var<storage, {}> g{}: array<{}>;\n",
            group, binding, Access::to_name(), self.g_ident, T::wgsl_name()
        );
        self.globals.push_str(&global_decl);

        // 2. Create the global variable handle
        let new_var = Var { 
            ident: self.g_ident, 
            _marker: PhantomData, 
            t: VarType::Global 
        };

        // 3. Prepend RuntimeArray<T> to the HList
        ShaderCode {
            g_ident: self.g_ident + 1,
            structs: self.structs,
            globals: self.globals,
            ext_functions: self.ext_functions,
            functions: self.functions,
            _state: PhantomData,
            uniforms: Cons {
                var: new_var,
                _key: PhantomData,
                tail: self.uniforms,
            },
        }
    }
    pub fn add_struct<T: ShaderStruct>(mut self) -> Self {
        let decl = T::wgsl_struct_decl();
        
        self.structs.push_str(&decl);
        self.structs.push('\n');
        
        self
    }
}
impl<Uniforms> ShaderCode<EmptyShader, Uniforms> {
    pub fn build_pipeline<F, FinalState>(self, builder_closure: F) -> String
    where
        F: FnOnce(ShaderCode<EmptyShader, Nil>, &Uniforms) -> ShaderCode<FinalState, Nil>,
    {
        let clean_builder = ShaderCode {
            g_ident: self.g_ident,
            structs: self.structs,
            globals: self.globals,
            ext_functions: self.ext_functions,
            functions: self.functions,
            uniforms: Nil,
            _state: PhantomData,
        };

        let final_builder = builder_closure(clean_builder, &self.uniforms);

        format!(
            "{}\n{}\n{}\n{}",
            final_builder.structs, final_builder.ext_functions.iter().map(|(_, x)|(**x).clone()).collect::<Vec<_>>().join("\n"), final_builder.globals, final_builder.functions
        )
    }
}
impl<U> ShaderCode<EmptyShader, U> {
    pub fn vert<'a, Args, Out, Body>(mut self, name: &str, body: Body) -> ShaderCode<Out, U>
    where
        Out: ShaderReturn + 'a + Clone,
        Body: ShaderClosure<'a, Args, Out>,
    {
        let (a, vs_code) = FunctionBuilder::vert(name, body);
        
        self.functions.push_str(&vs_code);
        self.functions.push_str("\n\n");
        self.ext_functions.extend(a.into_iter());
        
        ShaderCode {
            structs: self.structs,
            globals: self.globals,
            ext_functions: self.ext_functions,
            functions: self.functions,
            uniforms: self.uniforms, 
            _state: PhantomData,
            g_ident: self.g_ident,     
        }
    }
}
impl<VOut, U> ShaderCode<VOut, U> {
    pub fn frag<'a, Out, Body>(mut self, name: &str, body: Body) -> ShaderCode<Out, U>
    where
        VOut: ShaderParam, 
        Out: ShaderReturn + 'a + Clone,
        Body: ShaderClosure<'a, (VOut,), Out>,
    {
        let (fm, fs_code ) = FunctionBuilder::frag(name, body);
        
        self.functions.push_str(&fs_code);
        self.functions.push_str("\n\n");
        self.ext_functions.extend(fm.into_iter());
        
        ShaderCode {
            structs: self.structs,
            globals: self.globals,
            ext_functions: self.ext_functions,
            functions: self.functions,
            uniforms: self.uniforms,
            _state: PhantomData,
            g_ident: self.g_ident
        }
    }
}
impl<State, U> ShaderCode<State, U> {
    pub fn comp<'a, Args, Body>(
        mut self, 
        name: &str, 
        workgroup_size: (u32, u32, u32), 
        body: Body
    ) -> Self
    where
        Body: ShaderClosure<'a, Args, ()>,
    {
        let (fm, cs_code) = FunctionBuilder::compute(name, workgroup_size, body);
        
        self.ext_functions.extend(fm.into_iter());

        self.functions.push_str(&cs_code);
        self.functions.push_str("\n\n");
        
        self
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

#[derive(Clone)]
pub struct ShaderFn<Args, Ret> {
    pub name: &'static str,
    pub code: Arc<String>,
    deps: Arc<HashMap<&'static str, Arc<String>>>,
    _marker: std::marker::PhantomData<(Args, Ret)>,
}

pub trait ShaderFnParam {
    fn get_fn_params(ident: &mut i32) -> (Vec<String>, Self);
}

pub trait ShaderFnReturn {
    fn fn_return_decl() -> String;
    fn to_return_string(&self) -> String;
}
impl<T: WgslType> ShaderFnParam for Var<T> {
    fn get_fn_params(ident: &mut i32) -> (Vec<String>, Self) {
        let id = *ident;
        *ident += 1;
        (
            vec![format!("v{}: {}", id, T::wgsl_name())],
            Var { ident: id, _marker: std::marker::PhantomData, t: VarType::Local }
        )
    }
}

impl<T: WgslType> ShaderFnReturn for Var<T> {
    fn fn_return_decl() -> String { T::wgsl_name().to_string() }
    fn to_return_string(&self) -> String { self.to_string() }
}
pub trait ShaderFnClosure<'a, Args, Out> {
    fn build_and_evaluate(self, ident: &mut i32) -> (Vec<String>, ShaderDSL<'a, Out>);
}

impl<'a, Out : 'a, Func> ShaderFnClosure<'a, (), Out> for Func
where
    Func: Fn() -> ShaderDSL<'a, Out>,
{
    fn build_and_evaluate(self, _ident: &mut i32) -> (Vec<String>, ShaderDSL<'a, Out>) {
        (Vec::new(), self())
    }
}

macro_rules! impl_normal_closure {
    ( $($Param:ident),* ) => {
        #[allow(non_snake_case)]
        impl<'a, Out : 'a, Func, $($Param),*> ShaderFnClosure<'a, ($($Param,)*), Out> for Func
        where
            $($Param: ShaderFnParam,)* 
            Func: FnOnce($($Param),*) -> ShaderDSL<'a, Out>,
        {
            fn build_and_evaluate(self, ident: &mut i32) -> (Vec<String>, ShaderDSL<'a, Out>) {
                let mut all_decls = Vec::new();
                $(
                    let (mut decls, $Param) = $Param::get_fn_params(ident);
                    all_decls.append(&mut decls);
                )*
                (all_decls, self($($Param),*))
            }
        }
    };
}

impl_normal_closure!(A);
impl_normal_closure!(A, B);
impl_normal_closure!(A, B, C);

pub fn define_fn<'a, Args, Out, Body>(name: &'static str, body: Body) -> ShaderFn<Args, Out>
where
    Out: ShaderFnReturn + 'a + Clone,
    Body: ShaderFnClosure<'a, Args, Out>,
{
    let mut ident = 0;
    
    let (param_strs, program) = body.build_and_evaluate(&mut ident);
    let program_with_return = program.bind(move |out_val| {
        lift_f::<'_, _, _, ShaderDSL<'_, _>>(ShaderDSLF::Return(out_val.to_return_string(), ()))
    });
    
    let mut builder = FunctionBuilder::new();
    builder.ident = ident;
    let body_str = builder._build_shader(program_with_return);
    
    let code = format!("\nfn {}({}) -> {}\n{{{}\n}}\n", 
        name, param_strs.join(", "), Out::fn_return_decl(), body_str
    );
    
    ShaderFn { name, code: Arc::new(code), deps: Arc::new(builder.deps), _marker: std::marker::PhantomData }
}
impl ShaderFn<(), ()> {
    pub fn call<'a>(&self) -> ShaderDSL<'a, ()> {
        let name = self.name;
        mdo! {
            _call_func(FuncName::CallExtFn(name, self.code.clone(), self.deps.clone()), vec![]);
        }
    }
}
macro_rules! impl_typed_call_no_ret {
    ( $($arg:ident : $T:ident : $Expr:ident),* ) => {
        impl<$($T: WgslType,)*> ShaderFn<($(Var<$T>,)*), ()> {
            
            pub fn call<'a, $($Expr),*>(&self, $($arg: $Expr),*) -> ShaderDSL<'a, ()> 
                where $($Expr : Into<ShaderDSL<'a, Var<$T>>> + Clone + 'a),*,
                      $(Var<$T> : Into<FuncArg>),*,
                      $($T : 'a),*
            {
                let name = self.name;
                let code = self.code.clone();
                let deps = self.deps.clone();
                $(let $arg = Arc::new($arg);)*
                
                _mdo_move! {
                    [code, deps, $($arg),*]
                    
                    $($arg <- (*$arg).clone().into();)*
                    
                    _call_func(
                        FuncName::CallExtFn(name, code.clone(), deps.clone()), 
                        vec![ $($arg.into()),* ]
                    )
                }
            }
        }
    };
}
impl_typed_call_no_ret!(a: A : EA);
impl_typed_call_no_ret!(a: A : EA, b: B : EB);
impl_typed_call_no_ret!(a: A : EA, b: B : EB, c: C : EC);
impl_typed_call_no_ret!(a: A : EA, b: B : EB, c: C : EC, d: D : ED);

macro_rules! impl_typed_call_ret {
    ( $($arg:ident : $T:ident : $Expr:ident),* ) => {
        impl<$($T: WgslType,)* Ret : WgslType> ShaderFn<($(Var<$T>,)*), Var<Ret>> {
            
            pub fn call<'a, $($Expr),*>(&self, $($arg: $Expr),*) -> ShaderDSL<'a, Var<Ret>> 
                where $($Expr : Into<ShaderDSL<'a, Var<$T>>> + Clone + 'a),*,
                      $(Var<$T> : Into<FuncArg>),*,
                      $($T : 'a),*
            {
                let name = self.name;
                let code = self.code.clone();
                let deps = self.deps.clone();
                $(let $arg = Arc::new($arg);)*

                _mdo_move! {
                    [code, deps, $($arg),*]
                    
                    $($arg <- (*$arg).clone().into();)*
                    
                    ident <- _new_ident();
                    
                    _call_func_rt(
                        FuncName::CallExtFn(name, code.clone(), deps.clone()), 
                        vec![ $($arg.into()),* ], 
                        ident
                    )
                }
            }
        }
    };
}

impl_typed_call_ret!(a: A : EA);
impl_typed_call_ret!(a: A : EA, b: B : EB);
impl_typed_call_ret!(a: A : EA, b: B : EB, c: C : EC);
impl_typed_call_ret!(a: A : EA, b: B : EB, c: C : EC, d: D : ED);

#[derive(ShaderStruct)]
struct Camera {
    view_proj: Vec4<f32>,
    position: Vec3<f32>,
}

#[test]
fn test_shader_builder() {
    let tea = define_fn("tea", |v0: Var<f32>, v1: Var<f32>| mdo! { 
        make_float2!(v0, v1) 
    });
    struct CameraVar;
    struct PixelBufferVar;
    struct TimeVar;
    let final_wgsl = ShaderCode::new()
        .uniform::<CameraVar, Camera>(0, 0)
        .storage::<PixelBufferVar, Vec4<f32>, Array1D<Vec3<f32>>, ReadWrite>(0, 1)
        .build_pipeline(|builder, globals| {
            builder.comp(
                "cs_main", 
                (8, 8, 1), 
                |BuiltIn(global_id, _): BuiltIn<GlobalInvocationId, Vec3<u32>>| mdo! {
                    res <- tea.call(1., 2.);
                    _v <- globals.get(PixelBufferVar).at(0);
                    set(_v, make_float3!(1.));
                    _v1 <- make_float3!(1.);
                    _cam <- globals.get(CameraVar).in_context();
                    _pos <- _cam.position().clone().in_context();
                    Free::Pure(())
                }
            ).vert("vs_main", |Location(pos): Location<0, Vec3<f32>>|mdo!{
                _t <- pos.x().in_context();
                _cam <- globals.get(CameraVar).in_context();
                v <- make_float4!(1.);
                Free::Pure(Location::<0, _>::new(v))
            }).frag("f_main", |Location(color): Location<0, _>| mdo! {
                final_color <- color.xyzw().in_context();
                Free::Pure(Location::<0, _>::new(final_color))
            })
        });
    println!("{}", final_wgsl);
}