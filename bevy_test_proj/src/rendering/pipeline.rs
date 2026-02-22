// fn pipeline_creation(){
//     declare!(FragColor) // which will produce FragColorOutput used in Vertex Shader and FragColorInput in Fragment Shader

//     let shader_vert = 
//         |ctx: Context, in_pos: VertexAttr<Vec2>, in_color: VertexAttr<Vec3>, Transform(transform): Uniform<Mat4>| -> 
//             (FragColorOutput<Vec3>){
//                 ctx.position = make_vec4(in_pos, 0.0, 1.0);
//                 (in_color)
//     };
//     let shader_frag= | frag_color: FragColorInput<Vec3>, from_other_color_att: InputColorAttachment<Vec3>| -> (Vec3){
//         make_vec4(frag_color, 1.0)
//     };
//     let shader_vert = device.compile(shader_vert);
//     let shader_frag = device.compile(shader_frag);
//     let rp = RenderPassBuilder::new(|
//         subpass_builder, 
//         color_att_1: Read<ColorAttachment>, 
//         color_att_2: Write<Color_attachmemt>,
//         color_att_2: Write<Color_attachmemt>|{
//         subpass_builder.new()
//             .depth_stencil(some depth/stencil color attachment)
//             .vertex(shader_vert(VertPos, VertColor))
//             // uniforms can be changed in runtime, thus, in the creation stage it cannot be invoked
//             // it should be invoked in runtime.
//             .frag(shader_frag(color_att_1).output((color_att_2.multisample_to())))
//             .color_blend(|color: ColorAttachment|{ // must align with the output from shader_frag
//                 color.factor(...)
//             });
//             //.rasterizer with default
//             // in the next subpass it can read from the depth buffer, 
//             // and the compiler should inject subpass deps automatically
//     })
    
//     // code to store this RenderPass in somewhere
//     // code to create framebuffer
// }

// fn render_pipeline(){
//     CommandBuffer.scope(||{
//         fb.scope(|subpasses|{ // the framebuffer has bind to one render pass, 
//                               // so now it does not need to refer to it
//             subpasses[0].draw() // some draw command
//         })
//     })
// }

use std::{cell::RefCell, iter::Map, marker::PhantomData, ops::{Add, Deref, Div, Index, Mul}, rc::Rc};
use bevy::{mesh::{Indices, VertexAttributeValues}, platform::collections::HashMap, prelude::*};
use shader_macros::shader_lang;

struct ShaderLang{

}

// at creation stage, the lambda should detect vertex attr from closure, and uses uniforms as param

enum Op{
    MakeVec4,
    Assign
}
enum IR {
    Call,
    Binary,
    Unary,
    Literal,
    Access,
}
trait NumMarker : Sized + Add + Div + Mul{
    fn into_literal(self) -> LiteralNum;
    fn try_from_ctx(var: &CtxVar) -> anyhow::Result<Self>;
}
macro_rules! impl_num_marker {
    // Matches patterns like "i32 => I32"
    ($($type:ty => $variant:ident),* $(,)?) => {
        $(
            impl NumMarker for $type {
                fn into_literal(self) -> LiteralNum {
                    LiteralNum::$variant(self)
                }
                fn try_from_ctx(var: &CtxVar) -> anyhow::Result<Self> {
                    if let CtxVar::Literal(LiteralNum::$variant(val)) = var {
                        Ok(*val)
                    } else {
                        Err(anyhow::anyhow!("failed to convert"))
                    }
                }
            }
        )*
    };
}

// Apply the macro
impl_num_marker!(
    i32 => I32,
    u32 => U32,
    i64 => I64,
    u64 => U64,
    f32 => F32,
    i16 => I16,
    u16 => U16
);
enum LiteralNum{
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
}
enum BinaryOp {
    Plus,
    Minus,
    Divide,
    Mul
}
enum CtxVar{
    Literal(LiteralNum),
    Binary(BinaryOp, u32, u32),
    VertexBuffer(),
}
struct FunctionContext{
    variables: Vec<CtxVar>,
}
impl FunctionContext {
    fn new_var<T>(&mut self, i: T) -> usize
        where T : NumMarker
    {
        self.variables.push(CtxVar::Literal(i.into_literal()));
        self.variables.len() - 1
    }
    fn get(&self, i: usize) -> &CtxVar {
        &self.variables[i]
    }
}

struct PipelineBuilder{
    fb: FunctionContext,
     
}

fn test(asset_driver: Res<AssetServer>){
    
    let a: &[[f64; 3]] = &[[0., 0., 0.], [1., 1., 1.]];
    // two kinds of vertex shader
    // 1. binds the vertex shader to the self 
    // 2. binds the vertex shader at runtime

    // what proc-macro will do
    let shader_frag = ||{
        let a = 1;
        let b = 2;
        let c = &a + &b;
        let c = &c + &c;
    };
}

fn access_mesh_data(
    // CHANGED: Query for Mesh3d instead of Handle<Mesh>
    mesh_query: Query<(&Name, &Mesh3d), Added<Mesh3d>>, 
    meshes: Res<Assets<Mesh>>,
) {
    for (name, mesh_3d) in mesh_query.iter() {
        // CHANGED: Access the handle via .0
        let mesh_handle = &mesh_3d.0; 

        if let Some(mesh) = meshes.get(mesh_handle) {
            println!("Reading mesh data for entity: {:?}", name);

            // Access positions (Vertex data)
            if let Some(VertexAttributeValues::Float32x3(positions)) = 
                mesh.attribute(Mesh::ATTRIBUTE_POSITION) 
            {
                println!("Found {} vertices", positions.len());
            }

            // Access indices (Triangle data)
            if let Some(indices) = mesh.indices() {
                println!("Found indices for geometry");
            }
        }
    }
}