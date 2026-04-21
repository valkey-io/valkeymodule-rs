use super::InfoContextTrait;
use crate::{InfoContext, OneInfoSectionData, ValkeyResult};

impl InfoContextTrait for InfoContext {
    fn build_one_section(&self, data: OneInfoSectionData) -> ValkeyResult<()> {
        InfoContext::build_one_section(self, data)
    }
}
