#[cfg(not(any(miri, NO_UI_TESTS)))]
#[test]
fn compile_fail() {
    let test_cases = trybuild::TestCases::new();
    test_cases.compile_fail("tests/ui/compile-fail/pinned_drop/*.rs");
    test_cases.compile_fail("tests/ui/compile-fail/pin_data/*.rs");
    test_cases.compile_fail("tests/ui/compile-fail/init/*.rs");
    test_cases.compile_fail("tests/ui/compile-fail/zeroable/*.rs");
}

#[cfg(not(any(miri, NO_UI_TESTS)))]
#[test]
fn expand() {
    macrotest::expand("tests/ui/expand/*.rs");
}
