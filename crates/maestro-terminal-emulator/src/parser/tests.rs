//! Numeric accumulator presence through saturation and reuse.

use super::Parser;

#[test]
fn numeric_presence_never_wraps() {
    for digits in [255, 256, 257, 512] {
        let mut parser = Parser::init();
        for _ in 0..digits {
            parser.parameter(b'9');
        }
        assert_eq!(parser.param_acc, 65535);
        assert!(parser.numeric_present);
        parser.parameter(b';');
        assert!(!parser.numeric_present);
        assert_eq!(parser.param_acc, 0);
        parser.parameter(b'3');
        assert!(parser.numeric_present);
        assert_eq!(parser.param_acc, 3);
        parser.clear();
        assert!(!parser.numeric_present);
        assert_eq!(parser.param_acc, 0);
    }
}
