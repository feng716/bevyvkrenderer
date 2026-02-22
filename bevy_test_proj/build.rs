use std::{fs, process::Command};
use std::io;
use std::path::Path;

pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.as_ref().join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(entry.path(), dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
fn main(){
    println!("cargo::rerun-if-changed=shaders");
    let files = fs::read_dir("shaders").unwrap();
    for i in files {
        let i = i.unwrap();
        let file_path= i.file_name().into_string().unwrap();
        if file_path.ends_with("spv") {
            continue;
        }
        println!("cargo:warning={:?}", file_path);
        let file_path = i.path();
        let status = Command::new("glslc")
            .arg(&file_path)
            .arg("-o")
            .arg(file_path.with_extension("spv"))
            .status()
            .expect(format!("Failed to Compile Shaders {}", file_path.display()).as_str());
        if !status.success() {
            panic!("Failed to compile")
        }
    }
    copy_dir_all("shaders", "target/debug/shaders").unwrap();
}