use std::marker::PhantomData;

use ash::vk::BufferUsageFlags;

type Float3 = [f32; 3];
struct Position;
struct Normal;
type Float2 = [f32; 2];
enum VertexAttrFormat {
    Float,
    Float2,
    Float3,
    Float4,
}
struct Nil;

struct Cons<K, V, Tail, const STRIDE: u32> {
    index: u32,
    _key: PhantomData<K>,
    _val: PhantomData<V>,
    tail: Tail,
}
struct Var<T>
where
    T: Default,
{
    ref_to_fb: u32,
    marker: T,
}
struct Here;
struct There<T>(PhantomData<T>);

trait GetType<IsFound, Key> {
    type Type: Default;
    const STRIDE: u32;
}
impl<LookUpKey, Val, Tail, const STRIDE: u32> GetType<Here, LookUpKey> for Cons<LookUpKey, Val, Tail, STRIDE>
where
    Val: Default,
{
    type Type = Val;
    const STRIDE: u32 = STRIDE;
}
impl<LookUpKey, Key, Val, Tail, Rest, const STRIDE: u32> GetType<There<Rest>, LookUpKey> for Cons<Key, Val, Tail, STRIDE>
where
    Tail: GetType<Rest, LookUpKey>,
{
    type Type = Tail::Type;
    const STRIDE: u32 = STRIDE;
}

struct VertexData<T> {
    offset: usize,
    tail: PhantomData<T>,
}

impl<T> VertexData<T> {
    // fn cons<T1>(v: T1) -> VertexData<Cons<AddKey, AddValue, T, STRIDE>> {
    //     VertexData {
    //         tail: ,
    //         offset: self.offset
    //     }
    // }
    fn add<AddKey, AddValue, const STRIDE: u32>(self) -> VertexData<Cons<AddKey, AddValue, T, STRIDE>> {
        VertexData {
            tail: PhantomData,
            offset: self.offset
        }
            // Cons {
            //     index: 0,
            //     _key: PhantomData,
            //     _val: PhantomData,
            //     tail: self.tail,
            // }
    }
    fn get<LookUpKey, Rest>(&self, _key: LookUpKey) -> Var<T::Type>
    where
        T: GetType<Rest, LookUpKey>,
    {
        Var {
            ref_to_fb: 0,
            marker: Default::default(),
        }
    }
    fn get_stride<LookUpKey, Rest>(&self, _key: LookUpKey) -> u32
    where
        T: GetType<Rest, LookUpKey>,
    {
        T::STRIDE
    }
}

impl VertexData<Nil> {
    fn new<'a, T>(s: &'a [T], offset: usize) -> VertexData<Nil> {
        let create_info = ash::vk::BufferCreateInfo::default()
            .size(std::mem::size_of_val(s) as u64)
            .usage(BufferUsageFlags::VERTEX_BUFFER);
        VertexData {
            tail: PhantomData,
            offset: offset
        }
    }
}

#[test]
fn test() {
    let data = [[1, 2, 3], [4, 5, 6]];
    let test = VertexData::new(&data, 0)
        .add::<Position, Float2, 0>()
        .add::<Normal, Float3, 6>();
    let test2 = VertexData::new(&data, 0)
        .add::<Normal, Float2, 0>()
        .add::<Position, Float3, 1>();
    let _a = test.get(Normal);
    let _a = test.get_stride(Normal);
    assert_eq!(_a, 6)
}
