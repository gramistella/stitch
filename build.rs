fn main() {
    // If UI feature is off, don't invoke slint-build at all.
    // Cargo sets CARGO_FEATURE_<NAME> for enabled features.
    if std::env::var_os("CARGO_FEATURE_UI").is_none() {
        return;
    }

    // Compiles ui/app.slint and sets SLINT_INCLUDE_GENERATED for slint::include_modules!()
    slint_build::compile("ui/app.slint").expect("Failed to compile Slint UI");
}
