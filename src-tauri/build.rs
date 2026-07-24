fn main() {
    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;
        use tauri_build::WindowsAttributes;
        let mut windows = WindowsAttributes::new();
        windows = windows.app_manifest( r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
        "#);
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
        let nvapi_lib = manifest_dir.join("..").join("R560-developer").join("amd64").join("nvapi64.lib");
        let nvapi_lib_dir = nvapi_lib.parent().expect("nvapi64.lib parent");
        println!("cargo:rustc-link-search=native={}", nvapi_lib_dir.display());
        println!("cargo:rustc-link-lib=nvapi64");
        println!("cargo:rerun-if-changed={}", nvapi_lib.display());
        tauri_build::try_build(
            tauri_build::Attributes::new().windows_attributes(windows)
        ).expect("Failed to run build script");
    }
    #[cfg(not(target_os = "windows"))]
    {
        tauri_build::build();
    }
}
