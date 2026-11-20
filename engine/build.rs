//! Embeds the icons and the manifest into the exe.

fn main() {
    println!("cargo:rerun-if-changed=../assets/app.ico");
    println!("cargo:rerun-if-changed=../assets/tray.ico");
    winresource::WindowsResource::new()
        // lowest id = the exe's icon in Explorer
        .set_icon_with_id("../assets/app.ico", "1")
        .set_icon_with_id("../assets/tray.ico", "2")
        // DPI aware, so the tray icon is drawn at the real size instead of a blurry upscale
        .set_manifest(
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>"#,
        )
        .compile()
        .expect("embedding icons (needs the Windows SDK's rc.exe)");
}
