fn main() {
    // Compiles ui/app.slint and sets SLINT_INCLUDE_GENERATED for slint::include_modules!()
    slint_build::compile("ui/app.slint").expect("Failed to compile Slint UI");
}
