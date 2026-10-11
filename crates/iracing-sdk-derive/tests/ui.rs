// These globbed pass/compile-fail fixtures are the executable ownership boundary
// for tested input forms and compiler diagnostics. Keep .stderr expectations
// with the failing inputs rather than reproducing their messages in Markdown.
#[test]
fn public_rejection_contracts() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
    tests.pass("tests/pass/*.rs");
}
