//! 构建脚本：将图标嵌入 Windows 可执行文件。
//!
//! `winresource` 在编译期调用 rc.exe（或 windres）将 .rc 文件编译为 .res，
//! 链接器随后将其嵌入 exe 的资源段。

fn main() {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        eprintln!("WARNING: 无法读取 CARGO_MANIFEST_DIR");
        return;
    };
    let icon_path = std::path::Path::new(&manifest_dir)
        .join("assets")
        .join("icon.ico");

    if !icon_path.exists() {
        eprintln!("WARNING: 图标文件不存在: {}", icon_path.display());
        return;
    }

    let Some(icon_str) = icon_path.to_str() else {
        eprintln!("WARNING: 图标路径包含非法字符: {}", icon_path.display());
        return;
    };

    if let Err(e) = winresource::WindowsResource::new()
        .set_icon(icon_str)
        .compile()
    {
        eprintln!("WARNING: 嵌入图标失败: {e}");
    }
}
