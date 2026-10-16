//! Embedded brand input for browser qualification.
use maestro_theme::{ThemeInfo, ThemeOperations};
/// Supply only the checked-in brand pack, with no browser filesystem access.
pub struct Pack;
impl ThemeOperations for Pack {
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        Ok(include_str!("../../../../assets/brand/brand.json").into())
    }
    fn environment(&self, _: &str) -> Option<String> {
        unreachable!()
    }
    fn exists(&self, _: &str) -> bool {
        unreachable!()
    }
    fn read_dir(&self, _: &str) -> std::io::Result<Vec<String>> {
        unreachable!()
    }
    fn sort_by_name(&self, _: &mut [ThemeInfo]) -> std::io::Result<()> {
        unreachable!()
    }
}
