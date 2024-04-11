#[cfg_attr(not(any(miri, NO_UI_TESTS)), test)]
fn ui() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
