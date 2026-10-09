//! 把 assets/appicon.ico 嵌成 exe 圖示（檔案總管與工作列看到的那個）。只有 Windows 需要；
//! 失敗（例如找不到 Windows SDK 的 rc.exe）只發警告，exe 照樣編得出來、只是預設圖示。
fn main() {
    println!("cargo:rerun-if-changed=assets/appicon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/appicon.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=嵌入 exe 圖示失敗（{e}），exe 會是預設圖示");
        }
    }
}
