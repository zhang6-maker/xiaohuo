fn main() {
    tauri_build::build();
    // 前端 dist 变动时强制重编，确保 tauri://localhost 资源被重新嵌入
    println!("cargo:rerun-if-changed=../dist");
    println!("cargo:rerun-if-changed=../dist/index.html");
}
