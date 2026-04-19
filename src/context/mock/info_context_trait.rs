use crate::{OneInfoSectionData, ValkeyResult};

#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait InfoContextTrait {
    fn build_one_section(&self, data: OneInfoSectionData) -> ValkeyResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InfoContextFieldTopLevelData;

    fn sample_section() -> OneInfoSectionData {
        ("mysec".to_string(), InfoContextFieldTopLevelData::new())
    }

    #[test]
    fn test_dispatches_through_impl_and_dyn() {
        fn static_dispatch(ctx: &impl InfoContextTrait) {
            let _ = ctx.build_one_section(sample_section());
        }
        fn dynamic_dispatch(ctx: &dyn InfoContextTrait) {
            let _ = ctx.build_one_section(sample_section());
        }

        let mut ctx = MockInfoContextTrait::new();
        ctx.expect_build_one_section()
            .withf(|(name, fields)| name == "mysec" && fields.is_empty())
            .times(2)
            .returning(|_| Ok(()));

        static_dispatch(&ctx);
        dynamic_dispatch(&ctx);
    }
}
