use std::env;
use std::path::{Path, PathBuf};

fn main() {
    #[cfg(target_os = "linux")]
    {
        if let Ok(out_dir_str) = env::var("OUT_DIR") {
            let out_dir = PathBuf::from(out_dir_str);
            for (link_name, target) in [
                ("libxcb.so", "/usr/lib64/libxcb.so.1"),
                ("libxkbcommon.so", "/usr/lib64/libxkbcommon.so.0"),
                ("libxkbcommon-x11.so", "/usr/lib64/libxkbcommon-x11.so.0"),
                ("libfontconfig.so", "/usr/lib64/libfontconfig.so.1"),
            ] {
                let link_path = out_dir.join(link_name);
                let _ = std::fs::remove_file(&link_path);
                if Path::new(target).exists() {
                    let _ = std::os::unix::fs::symlink(target, &link_path);
                }
            }
            println!("cargo:rustc-link-search=native={}", out_dir.display());
        }
    }
}
